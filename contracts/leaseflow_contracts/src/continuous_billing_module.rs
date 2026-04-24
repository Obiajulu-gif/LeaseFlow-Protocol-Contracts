use soroban_sdk::{contract, contractimpl, Address, Env, Map, Symbol, Vec, i128, u64, u32};
use soroban_sdk::token::Client as TokenClient;

// Contract state keys
const ADMIN: Symbol = Symbol::short("ADMIN");
const BILLING_CYCLES: Symbol = Symbol::short("BILLING");
const ACTIVE_LEASES: Symbol = Symbol::short("ACTIVE_LEASES");
const BILLING_CONFIG: Symbol = Symbol::short("BILLING_CFG");
const CONTRACT_VERSION: Symbol = Symbol::short("VERSION");

// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    Unauthorized = 1,
    LeaseNotFound = 2,
    InvalidInput = 3,
    BillingCycleNotFound = 4,
    AlreadyProcessed = 5,
    InsufficientFunds = 6,
    TransferFailed = 7,
}

// Billing cycle structure
#[derive(Clone)]
#[contracttype]
pub struct BillingCycle {
    pub cycle_id: u64,
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub asset_address: Address,
    pub rent_amount: i128,
    pub token: Address,
    pub cycle_start: u64,
    pub cycle_end: u64,
    pub status: String, // "pending", "processed", "failed", "expired"
    pub processed_at: Option<u64>,
    pub payment_received: bool,
    pub arrears_amount: i128,
}

// Active lease structure
#[derive(Clone)]
#[contracttype]
pub struct ActiveLease {
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub asset_address: Address,
    pub rent_amount: i128,
    pub token: Address,
    pub start_time: u64,
    pub end_time: u64,
    pub billing_frequency: u64, // seconds between billing cycles
    pub last_billing_cycle: u64,
    pub status: String, // "active", "expired", "terminated"
    pub total_owed: i128,
    pub total_paid: i128,
}

// Billing configuration
#[derive(Clone)]
#[contracttype]
pub struct BillingConfig {
    pub grace_period: u64, // seconds after due date before penalties
    pub late_fee_percentage: u32, // basis points
    pub max_arrears_threshold: i128,
    pub auto_process_enabled: bool,
}

// Events
#[contractevent]
pub struct BillingCycleProcessed {
    pub cycle_id: u64,
    pub lease_id: u64,
    pub rent_amount: i128,
    pub processed_at: u64,
    pub payment_received: bool,
}

#[contractevent]
pub struct LeaseBillingStarted {
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub rent_amount: i128,
    pub billing_frequency: u64,
    pub timestamp: u64,
}

#[contractevent]
pub struct ArrearsAccumulated {
    pub lease_id: u64,
    pub lessee: Address,
    pub arrears_amount: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct PaymentReceived {
    pub lease_id: u64,
    pub payer: Address,
    pub amount: i128,
    pub timestamp: u64,
}

pub struct ContinuousBillingModule;

#[contractimpl]
impl ContinuousBillingModule {
    /// Initialize the Continuous Billing Module
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(ContractError::Unauthorized);
        }
        
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&CONTRACT_VERSION, &"1.0.0");
        
        // Initialize storage
        let billing_cycles: Map<u64, BillingCycle> = Map::new(&env);
        let active_leases: Map<u64, ActiveLease> = Map::new(&env);
        
        env.storage().instance().set(&BILLING_CYCLES, &billing_cycles);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Set default billing configuration
        let config = BillingConfig {
            grace_period: 86400, // 24 hours
            late_fee_percentage: 500, // 5% in basis points
            max_arrears_threshold: 86400000, // 1000 tokens (assuming 8 decimals)
            auto_process_enabled: true,
        };
        env.storage().instance().set(&BILLING_CONFIG, &config);
        
        Ok(())
    }

    /// Register a new lease for continuous billing
    pub fn register_lease_billing(
        env: Env,
        lease_id: u64,
        lessor: Address,
        lessee: Address,
        asset_address: Address,
        rent_amount: i128,
        token: Address,
        start_time: u64,
        end_time: u64,
        billing_frequency: u64,
    ) -> Result<(), ContractError> {
        // Note: This module does NOT check escrow freeze status
        // It must continue operating even during escrow freezes
        
        let active_lease = ActiveLease {
            lease_id,
            lessor: lessor.clone(),
            lessee: lessee.clone(),
            asset_address,
            rent_amount,
            token: token.clone(),
            start_time,
            end_time,
            billing_frequency,
            last_billing_cycle: 0,
            status: String::from_str(&env, "active"),
            total_owed: 0,
            total_paid: 0,
        };
        
        // Store active lease
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        active_leases.set(lease_id, active_lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Emit event
        env.events().publish(
            (Symbol::short("LEASE_BILLING_STARTED"), lease_id),
            LeaseBillingStarted {
                lease_id,
                lessor,
                lessee,
                rent_amount,
                billing_frequency,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Process billing cycle for a lease
    pub fn process_billing_cycle(
        env: Env,
        lease_id: u64,
        processor: Address,
    ) -> Result<u64, ContractError> {
        // Note: This module does NOT check escrow freeze status
        // It must continue operating even during escrow freezes
        
        let current_time = env.ledger().timestamp();
        
        // Get active lease
        let active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        // Check if lease is still active
        if lease.status != String::from_str(&env, "active") {
            return Err(ContractError::InvalidInput);
        }
        
        // Calculate next billing cycle time
        let next_cycle_time = if lease.last_billing_cycle == 0 {
            lease.start_time
        } else {
            lease.last_billing_cycle + lease.billing_frequency
        };
        
        // Check if it's time to process
        if current_time < next_cycle_time {
            return Err(ContractError::InvalidInput);
        }
        
        // Create billing cycle
        let cycle_id = env.ledger().sequence();
        let billing_cycle = BillingCycle {
            cycle_id,
            lease_id,
            lessor: lease.lessor.clone(),
            lessee: lease.lessee.clone(),
            asset_address: lease.asset_address,
            rent_amount: lease.rent_amount,
            token: lease.token.clone(),
            cycle_start: next_cycle_time,
            cycle_end: next_cycle_time + lease.billing_frequency,
            status: String::from_str(&env, "pending"),
            processed_at: None,
            payment_received: false,
            arrears_amount: 0,
        };
        
        // Store billing cycle
        let mut billing_cycles: Map<u64, BillingCycle> = env.storage().instance()
            .get(&BILLING_CYCLES)
            .unwrap_or(Map::new(&env));
        billing_cycles.set(cycle_id, billing_cycle);
        env.storage().instance().set(&BILLING_CYCLES, &billing_cycles);
        
        // Update lease
        lease.last_billing_cycle = next_cycle_time;
        lease.total_owed += lease.rent_amount;
        active_leases.set(lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        Ok(cycle_id)
    }

    /// Process payment for a billing cycle
    pub fn process_payment(
        env: Env,
        payer: Address,
        cycle_id: u64,
        payment_amount: i128,
    ) -> Result<(), ContractError> {
        // Note: This module does NOT check escrow freeze status
        // It must continue operating even during escrow freezes
        
        payer.require_auth();
        
        let current_time = env.ledger().timestamp();
        
        // Get billing cycle
        let mut billing_cycles: Map<u64, BillingCycle> = env.storage().instance()
            .get(&BILLING_CYCLES)
            .unwrap_or(Map::new(&env));
        let mut cycle: BillingCycle = billing_cycles.get(cycle_id)
            .ok_or(ContractError::BillingCycleNotFound)?;
        
        // Check if already processed
        if cycle.status == String::from_str(&env, "processed") {
            return Err(ContractError::AlreadyProcessed);
        }
        
        // Validate payment amount
        if payment_amount > cycle.rent_amount {
            return Err(ContractError::InvalidInput);
        }
        
        // Transfer payment to lessor
        let token_client = TokenClient::new(&env, &cycle.token);
        token_client.transfer(&payer, &cycle.lessor, &payment_amount);
        
        // Update cycle
        cycle.payment_received = true;
        cycle.processed_at = Some(current_time);
        if payment_amount == cycle.rent_amount {
            cycle.status = String::from_str(&env, "processed");
        } else {
            cycle.status = String::from_str(&env, "failed");
            cycle.arrears_amount = cycle.rent_amount - payment_amount;
        }
        billing_cycles.set(cycle_id, cycle);
        env.storage().instance().set(&BILLING_CYCLES, &billing_cycles);
        
        // Update active lease
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(cycle.lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        lease.total_paid += payment_amount;
        if cycle.arrears_amount > 0 {
            lease.total_owed += cycle.arrears_amount;
            
            // Emit arrears event
            env.events().publish(
                (Symbol::short("ARREARS_ACCUMULATED"), cycle.lease_id),
                ArrearsAccumulated {
                    lease_id: cycle.lease_id,
                    lessee: cycle.lessee.clone(),
                    arrears_amount: cycle.arrears_amount,
                    timestamp: current_time,
                }
            );
        }
        
        active_leases.set(cycle.lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Emit payment event
        env.events().publish(
            (Symbol::short("PAYMENT_RECEIVED"), cycle_id),
            PaymentReceived {
                lease_id: cycle.lease_id,
                payer,
                amount: payment_amount,
                timestamp: current_time,
            }
        );
        
        // Emit billing cycle processed event
        env.events().publish(
            (Symbol::short("BILLING_CYCLE_PROCESSED"), cycle_id),
            BillingCycleProcessed {
                cycle_id,
                lease_id: cycle.lease_id,
                rent_amount: cycle.rent_amount,
                processed_at: current_time,
                payment_received: cycle.payment_received,
            }
        );
        
        Ok(())
    }

    /// Terminate lease billing
    pub fn terminate_lease_billing(
        env: Env,
        lease_id: u64,
        terminator: Address,
    ) -> Result<(), ContractError> {
        // Note: This module does NOT check escrow freeze status
        // It must continue operating even during escrow freezes
        
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        // Validate terminator (lessor or admin)
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if terminator != lease.lessor && terminator != admin {
            return Err(ContractError::Unauthorized);
        }
        terminator.require_auth();
        
        // Update lease status
        lease.status = String::from_str(&env, "terminated");
        active_leases.set(lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        Ok(())
    }

    /// Get active lease information
    pub fn get_active_lease(env: Env, lease_id: u64) -> Result<ActiveLease, ContractError> {
        let active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        
        active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)
    }

    /// Get billing cycle information
    pub fn get_billing_cycle(env: Env, cycle_id: u64) -> Result<BillingCycle, ContractError> {
        let billing_cycles: Map<u64, BillingCycle> = env.storage().instance()
            .get(&BILLING_CYCLES)
            .unwrap_or(Map::new(&env));
        
        billing_cycles.get(cycle_id)
            .ok_or(ContractError::BillingCycleNotFound)
    }

    /// Get billing configuration
    pub fn get_billing_config(env: Env) -> BillingConfig {
        env.storage().instance()
            .get(&BILLING_CONFIG)
            .unwrap_or_else(|| BillingConfig {
                grace_period: 86400,
                late_fee_percentage: 500,
                max_arrears_threshold: 86400000,
                auto_process_enabled: true,
            })
    }

    /// Update billing configuration (admin only)
    pub fn update_billing_config(
        env: Env,
        admin: Address,
        config: BillingConfig,
    ) -> Result<(), ContractError> {
        let contract_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if contract_admin != admin {
            return Err(ContractError::Unauthorized);
        }
        admin.require_auth();
        
        env.storage().instance().set(&BILLING_CONFIG, &config);
        Ok(())
    }

    /// Get contract version
    pub fn get_version(env: Env) -> String {
        env.storage().instance()
            .get(&CONTRACT_VERSION)
            .unwrap_or_else(|| "1.0.0".into_val(&env))
    }

    /// Check if lease has expired (for mathematical resolution after freeze)
    pub fn check_lease_expiration(env: Env, lease_id: u64) -> Result<bool, ContractError> {
        let lease: ActiveLease = Self::get_active_lease(env, lease_id)?;
        let current_time = env.ledger().timestamp();
        
        Ok(current_time >= lease.end_time)
    }

    /// Calculate final settlement for expired lease
    pub fn calculate_final_settlement(env: Env, lease_id: u64) -> Result<(i128, i128), ContractError> {
        let lease: ActiveLease = Self::get_active_lease(env, lease_id)?;
        
        // Calculate remaining rent due
        let current_time = env.ledger().timestamp();
        let remaining_cycles = if current_time >= lease.end_time {
            0
        } else {
            let remaining_time = lease.end_time - current_time;
            (remaining_time + lease.billing_frequency - 1) / lease.billing_frequency
        };
        
        let remaining_rent = remaining_cycles as i128 * lease.rent_amount;
        let current_arrears = lease.total_owed - lease.total_paid;
        
        Ok((remaining_rent, current_arrears))
    }
}
