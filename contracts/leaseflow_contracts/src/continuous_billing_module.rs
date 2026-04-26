use soroban_sdk::{contract, contractimpl, Address, Env, Map, Symbol, Vec, i128, u64, u32, BytesN};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::auth::ContractAuth;
use soroban_sdk::crypto::sha256;

// Contract state keys
const ADMIN: Symbol = Symbol::short("ADMIN");
const BILLING_CYCLES: Symbol = Symbol::short("BILLING");
const ACTIVE_LEASES: Symbol = Symbol::short("ACTIVE_LEASES");
const BILLING_CONFIG: Symbol = Symbol::short("BILLING_CFG");
const CONTRACT_VERSION: Symbol = Symbol::short("VERSION");
const RENT_TREASURY: Symbol = Symbol::short("RENT_TREAS");
const AUTHORIZATION_NONCES: Symbol = Symbol::short("AUTH_NONCES");
const REENTRANCY_GUARD: Symbol = Symbol::short("REENT_GUARD");
const BILLING_STATE: Symbol = Symbol::short("BILL_STATE");

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
    ReentrancyDetected = 8,
    AuthorizationExpired = 9,
    AuthorizationInvalid = 10,
    TreasuryInsufficient = 11,
    InvalidTimestamp = 12,
    LeaseNotActive = 13,
    BillingNotEnabled = 14,
}

// Enhanced billing cycle structure with rent_per_second support
#[derive(Clone)]
#[contracttype]
pub struct BillingCycle {
    pub cycle_id: u64,
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub asset_address: Address,
    pub rent_amount: i128,
    pub rent_per_second: i128,
    pub token: Address,
    pub cycle_start: u64,
    pub cycle_end: u64,
    pub status: String, // "pending", "processed", "failed", "expired"
    pub processed_at: Option<u64>,
    pub payment_received: bool,
    pub arrears_amount: i128,
    pub actual_duration_seconds: u64,
    pub next_billing_date: u64,
}

// Enhanced active lease structure with continuous billing support
#[derive(Clone)]
#[contracttype]
pub struct ActiveLease {
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub asset_address: Address,
    pub rent_amount: i128,
    pub rent_per_second: i128,
    pub token: Address,
    pub start_time: u64,
    pub end_time: u64,
    pub billing_frequency: u64, // seconds between billing cycles
    pub last_billing_cycle: u64,
    pub status: String, // "active", "expired", "terminated"
    pub total_owed: i128,
    pub total_paid: i128,
    pub billing_enabled: bool,
    pub authorization_enabled: bool,
    pub max_authorized_amount: i128,
    pub current_authorized_amount: i128,
    pub authorization_expiry: u64,
    pub rent_treasury_address: Address,
}

// Enhanced billing configuration with continuous billing support
#[derive(Clone)]
#[contracttype]
pub struct BillingConfig {
    pub grace_period: u64, // seconds after due date before penalties
    pub late_fee_percentage: u32, // basis points
    pub max_arrears_threshold: i128,
    pub auto_process_enabled: bool,
    pub continuous_billing_enabled: bool,
    pub min_billing_interval: u64, // minimum seconds between billing cycles
    pub max_billing_interval: u64, // maximum seconds between billing cycles
    pub reentrancy_protection_enabled: bool,
}

// Rent Treasury structure for separating operational revenue from escrow
#[derive(Clone)]
#[contracttype]
pub struct RentTreasury {
    pub treasury_address: Address,
    pub total_collected: i128,
    pub total_distributed: i128,
    pub available_balance: i128,
    pub last_collection_time: u64,
    pub collection_count: u64,
}

// Authorization structure for pull-based payments
#[derive(Clone)]
#[contracttype]
pub struct PaymentAuthorization {
    pub lease_id: u64,
    pub lessee: Address,
    pub authorized_amount: i128,
    pub expiry_timestamp: u64,
    pub nonce: u64,
    pub signature: BytesN<64>,
    pub created_at: u64,
    pub is_active: bool,
}

// Billing state tracking
#[derive(Clone)]
#[contracttype]
pub struct BillingState {
    pub global_billing_enabled: bool,
    pub emergency_pause: bool,
    pub pause_reason: Option<String>,
    pub pause_timestamp: Option<u64>,
    pub total_cycles_processed: u64,
    pub total_rent_collected: i128,
    pub last_process_time: u64,
}

// Events
#[contractevent]
pub struct RentPaymentExecuted {
    pub lease_id: u64,
    pub cycle_id: u64,
    pub amount: i128,
    pub lessee: Address,
    pub lessor: Address,
    pub treasury_address: Address,
    pub timestamp: u64,
    pub billing_duration: u64,
}

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
    pub rent_per_second: i128,
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

#[contractevent]
pub struct AuthorizationGranted {
    pub lease_id: u64,
    pub lessee: Address,
    pub authorized_amount: i128,
    pub expiry_timestamp: u64,
    pub timestamp: u64,
}

#[contractevent]
pub struct TreasuryUpdated {
    pub treasury_address: Address,
    pub total_collected: i128,
    pub available_balance: i128,
    pub timestamp: u64,
}

pub struct ContinuousBillingModule;

#[contractimpl]
impl ContinuousBillingModule {
    /// Initialize the Continuous Billing Module with enhanced features
    pub fn initialize(env: Env, admin: Address, rent_treasury_address: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(ContractError::Unauthorized);
        }
        
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&CONTRACT_VERSION, &"2.0.0");
        
        // Initialize storage
        let billing_cycles: Map<u64, BillingCycle> = Map::new(&env);
        let active_leases: Map<u64, ActiveLease> = Map::new(&env);
        let authorization_nonces: Map<Address, u64> = Map::new(&env);
        
        env.storage().instance().set(&BILLING_CYCLES, &billing_cycles);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        env.storage().instance().set(&AUTHORIZATION_NONCES, &authorization_nonces);
        
        // Initialize reentrancy guard
        env.storage().instance().set(&REENTRANCY_GUARD, &false);
        
        // Initialize rent treasury
        let treasury = RentTreasury {
            treasury_address: rent_treasury_address.clone(),
            total_collected: 0,
            total_distributed: 0,
            available_balance: 0,
            last_collection_time: env.ledger().timestamp(),
            collection_count: 0,
        };
        env.storage().instance().set(&RENT_TREASURY, &treasury);
        
        // Initialize billing state
        let billing_state = BillingState {
            global_billing_enabled: true,
            emergency_pause: false,
            pause_reason: None,
            pause_timestamp: None,
            total_cycles_processed: 0,
            total_rent_collected: 0,
            last_process_time: env.ledger().timestamp(),
        };
        env.storage().instance().set(&BILLING_STATE, &billing_state);
        
        // Set enhanced default billing configuration
        let config = BillingConfig {
            grace_period: 86400, // 24 hours
            late_fee_percentage: 500, // 5% in basis points
            max_arrears_threshold: 86400000, // 1000 tokens (assuming 8 decimals)
            auto_process_enabled: true,
            continuous_billing_enabled: true,
            min_billing_interval: 3600, // 1 hour minimum
            max_billing_interval: 2592000, // 30 days maximum
            reentrancy_protection_enabled: true,
        };
        env.storage().instance().set(&BILLING_CONFIG, &config);
        
        Ok(())
    }

    /// Register a new lease for continuous billing with rent_per_second support
    pub fn register_lease_billing(
        env: Env,
        lease_id: u64,
        lessor: Address,
        lessee: Address,
        asset_address: Address,
        rent_amount: i128,
        rent_per_second: i128,
        token: Address,
        start_time: u64,
        end_time: u64,
        billing_frequency: u64,
    ) -> Result<(), ContractError> {
        // Check if billing is enabled globally
        let billing_state: BillingState = env.storage().instance()
            .get(&BILLING_STATE)
            .unwrap_or_else(|| BillingState {
                global_billing_enabled: false,
                emergency_pause: false,
                pause_reason: None,
                pause_timestamp: None,
                total_cycles_processed: 0,
                total_rent_collected: 0,
                last_process_time: 0,
            });
        
        if !billing_state.global_billing_enabled || billing_state.emergency_pause {
            return Err(ContractError::BillingNotEnabled);
        }
        
        // Get rent treasury address
        let treasury: RentTreasury = env.storage().instance()
            .get(&RENT_TREASURY)
            .ok_or(ContractError::TreasuryInsufficient)?;
        
        let active_lease = ActiveLease {
            lease_id,
            lessor: lessor.clone(),
            lessee: lessee.clone(),
            asset_address,
            rent_amount,
            rent_per_second,
            token: token.clone(),
            start_time,
            end_time,
            billing_frequency,
            last_billing_cycle: 0,
            status: String::from_str(&env, "active"),
            total_owed: 0,
            total_paid: 0,
            billing_enabled: true,
            authorization_enabled: false,
            max_authorized_amount: 0,
            current_authorized_amount: 0,
            authorization_expiry: 0,
            rent_treasury_address: treasury.treasury_address,
        };
        
        // Store active lease
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        active_leases.set(lease_id, active_lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Emit enhanced event
        env.events().publish(
            (Symbol::short("LEASE_BILLING_STARTED"), lease_id),
            LeaseBillingStarted {
                lease_id,
                lessor,
                lessee,
                rent_amount,
                rent_per_second,
                billing_frequency,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Process billing cycle for a lease with enhanced security and rent_per_second calculation
    pub fn process_billing_cycle(
        env: Env,
        lease_id: u64,
        processor: Address,
    ) -> Result<u64, ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
        let current_time = env.ledger().timestamp();
        
        // Get active lease
        let active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        // Check if lease is still active and billing is enabled
        if lease.status != String::from_str(&env, "active") || !lease.billing_enabled {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::LeaseNotActive);
        }
        
        // Calculate next billing cycle time
        let next_cycle_time = if lease.last_billing_cycle == 0 {
            lease.start_time
        } else {
            lease.last_billing_cycle + lease.billing_frequency
        };
        
        // Check if it's time to process
        if current_time < next_cycle_time {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::InvalidTimestamp);
        }
        
        // Calculate actual duration and rent amount using rent_per_second
        let actual_duration = if current_time > lease.end_time {
            lease.end_time - next_cycle_time
        } else {
            current_time - next_cycle_time
        };
        
        let calculated_rent = if lease.rent_per_second > 0 {
            lease.rent_per_second * actual_duration as i128
        } else {
            lease.rent_amount
        };
        
        // Create enhanced billing cycle
        let cycle_id = env.ledger().sequence();
        let billing_cycle = BillingCycle {
            cycle_id,
            lease_id,
            lessor: lease.lessor.clone(),
            lessee: lease.lessee.clone(),
            asset_address: lease.asset_address,
            rent_amount: calculated_rent,
            rent_per_second: lease.rent_per_second,
            token: lease.token.clone(),
            cycle_start: next_cycle_time,
            cycle_end: next_cycle_time + lease.billing_frequency,
            status: String::from_str(&env, "pending"),
            processed_at: None,
            payment_received: false,
            arrears_amount: 0,
            actual_duration_seconds: actual_duration,
            next_billing_date: next_cycle_time + lease.billing_frequency,
        };
        
        // Store billing cycle
        let mut billing_cycles: Map<u64, BillingCycle> = env.storage().instance()
            .get(&BILLING_CYCLES)
            .unwrap_or(Map::new(&env));
        billing_cycles.set(cycle_id, billing_cycle);
        env.storage().instance().set(&BILLING_CYCLES, &billing_cycles);
        
        // Update lease
        lease.last_billing_cycle = next_cycle_time;
        lease.total_owed += calculated_rent;
        active_leases.set(lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Update billing state
        let mut billing_state: BillingState = env.storage().instance()
            .get(&BILLING_STATE)
            .unwrap_or_else(|| BillingState {
                global_billing_enabled: true,
                emergency_pause: false,
                pause_reason: None,
                pause_timestamp: None,
                total_cycles_processed: 0,
                total_rent_collected: 0,
                last_process_time: current_time,
            });
        billing_state.total_cycles_processed += 1;
        billing_state.last_process_time = current_time;
        env.storage().instance().set(&BILLING_STATE, &billing_state);
        
        Self::exit_reentrancy_guard(&env);
        Ok(cycle_id)
    }

    /// Process payment for a billing cycle with authorization and treasury support
    pub fn process_payment(
        env: Env,
        payer: Address,
        cycle_id: u64,
        payment_amount: i128,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
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
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::AlreadyProcessed);
        }
        
        // Validate payment amount
        if payment_amount > cycle.rent_amount {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::InvalidInput);
        }
        
        // Get rent treasury
        let mut treasury: RentTreasury = env.storage().instance()
            .get(&RENT_TREASURY)
            .ok_or(ContractError::TreasuryInsufficient)?;
        
        // Transfer payment to rent treasury (not directly to lessor)
        let token_client = TokenClient::new(&env, &cycle.token);
        token_client.transfer(&payer, &treasury.treasury_address, &payment_amount);
        
        // Update treasury
        treasury.total_collected += payment_amount;
        treasury.available_balance += payment_amount;
        treasury.collection_count += 1;
        treasury.last_collection_time = current_time;
        env.storage().instance().set(&RENT_TREASURY, &treasury);
        
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
        
        // Update billing state
        let mut billing_state: BillingState = env.storage().instance()
            .get(&BILLING_STATE)
            .unwrap_or_else(|| BillingState {
                global_billing_enabled: true,
                emergency_pause: false,
                pause_reason: None,
                pause_timestamp: None,
                total_cycles_processed: 0,
                total_rent_collected: 0,
                last_process_time: current_time,
            });
        billing_state.total_rent_collected += payment_amount;
        env.storage().instance().set(&BILLING_STATE, &billing_state);
        
        // Emit enhanced payment events
        env.events().publish(
            (Symbol::short("PAYMENT_RECEIVED"), cycle_id),
            PaymentReceived {
                lease_id: cycle.lease_id,
                payer,
                amount: payment_amount,
                timestamp: current_time,
            }
        );
        
        env.events().publish(
            (Symbol::short("RENT_PAYMENT_EXECUTED"), cycle_id),
            RentPaymentExecuted {
                lease_id: cycle.lease_id,
                cycle_id,
                amount: payment_amount,
                lessee: cycle.lessee.clone(),
                lessor: cycle.lessor.clone(),
                treasury_address: treasury.treasury_address,
                timestamp: current_time,
                billing_duration: cycle.actual_duration_seconds,
            }
        );
        
        env.events().publish(
            (Symbol::short("TREASURY_UPDATED"), cycle_id),
            TreasuryUpdated {
                treasury_address: treasury.treasury_address,
                total_collected: treasury.total_collected,
                available_balance: treasury.available_balance,
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
        
        Self::exit_reentrancy_guard(&env);
        Ok(())
    }

    /// Grant pull-based payment authorization using Soroban authorization payloads
    pub fn grant_payment_authorization(
        env: Env,
        lease_id: u64,
        lessee: Address,
        authorized_amount: i128,
        expiry_timestamp: u64,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
        lessee.require_auth();
        
        let current_time = env.ledger().timestamp();
        
        // Validate expiry
        if expiry_timestamp <= current_time {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::AuthorizationExpired);
        }
        
        // Get and update nonce
        let mut nonces: Map<Address, u64> = env.storage().instance()
            .get(&AUTHORIZATION_NONCES)
            .unwrap_or(Map::new(&env));
        let nonce = nonces.get(lessee.clone()).unwrap_or(0) + 1;
        nonces.set(lessee.clone(), nonce);
        env.storage().instance().set(&AUTHORIZATION_NONCES, &nonces);
        
        // Create authorization
        let authorization = PaymentAuthorization {
            lease_id,
            lessee: lessee.clone(),
            authorized_amount,
            expiry_timestamp,
            nonce,
            signature,
            created_at: current_time,
            is_active: true,
        };
        
        // Store authorization
        let auth_key = Symbol::short(&env, format!("AUTH_{}_{}", lease_id, nonce));
        env.storage().instance().set(&auth_key, &authorization);
        
        // Update lease authorization settings
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        lease.authorization_enabled = true;
        lease.max_authorized_amount = authorized_amount;
        lease.current_authorized_amount = authorized_amount;
        lease.authorization_expiry = expiry_timestamp;
        
        active_leases.set(lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        // Emit authorization event
        env.events().publish(
            (Symbol::short("AUTHORIZATION_GRANTED"), lease_id),
            AuthorizationGranted {
                lease_id,
                lessee,
                authorized_amount,
                expiry_timestamp,
                timestamp: current_time,
            }
        );
        
        Self::exit_reentrancy_guard(&env);
        Ok(())
    }
    
    /// Execute pull-based payment using authorization
    pub fn execute_pull_payment(
        env: Env,
        lease_id: u64,
        cycle_id: u64,
        nonce: u64,
        signature: BytesN<64>,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
        let current_time = env.ledger().timestamp();
        
        // Get authorization
        let auth_key = Symbol::short(&env, format!("AUTH_{}_{}", lease_id, nonce));
        let auth: PaymentAuthorization = env.storage().instance()
            .get(&auth_key)
            .ok_or(ContractError::AuthorizationInvalid)?;
        
        // Validate authorization
        if !auth.is_active || auth.expiry_timestamp < current_time {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::AuthorizationExpired);
        }
        
        // Get billing cycle
        let billing_cycles: Map<u64, BillingCycle> = env.storage().instance()
            .get(&BILLING_CYCLES)
            .unwrap_or(Map::new(&env));
        let cycle: BillingCycle = billing_cycles.get(cycle_id)
            .ok_or(ContractError::BillingCycleNotFound)?;
        
        // Validate amount against authorization
        if cycle.rent_amount > auth.authorized_amount {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::AuthorizationInvalid);
        }
        
        // Deactivate authorization after use
        let mut updated_auth = auth;
        updated_auth.is_active = false;
        env.storage().instance().set(&auth_key, &updated_auth);
        
        // Process payment using the existing payment function
        Self::exit_reentrancy_guard(&env);
        Self::process_payment(env, auth.lessee, cycle_id, cycle.rent_amount)
    }
    
    /// Reentrancy protection functions
    fn enter_reentrancy_guard(env: &Env) -> Result<(), ContractError> {
        let config: BillingConfig = env.storage().instance()
            .get(&BILLING_CONFIG)
            .unwrap_or_else(|| BillingConfig {
                grace_period: 86400,
                late_fee_percentage: 500,
                max_arrears_threshold: 86400000,
                auto_process_enabled: true,
                continuous_billing_enabled: true,
                min_billing_interval: 3600,
                max_billing_interval: 2592000,
                reentrancy_protection_enabled: true,
            });
        
        if config.reentrancy_protection_enabled {
            let guard_active: bool = env.storage().instance()
                .get(&REENTRANCY_GUARD)
                .unwrap_or(false);
            
            if guard_active {
                return Err(ContractError::ReentrancyDetected);
            }
            
            env.storage().instance().set(&REENTRANCY_GUARD, &true);
        }
        
        Ok(())
    }
    
    fn exit_reentrancy_guard(env: &Env) {
        let config: BillingConfig = env.storage().instance()
            .get(&BILLING_CONFIG)
            .unwrap_or_else(|| BillingConfig {
                grace_period: 86400,
                late_fee_percentage: 500,
                max_arrears_threshold: 86400000,
                auto_process_enabled: true,
                continuous_billing_enabled: true,
                min_billing_interval: 3600,
                max_billing_interval: 2592000,
                reentrancy_protection_enabled: true,
            });
        
        if config.reentrancy_protection_enabled {
            env.storage().instance().set(&REENTRANCY_GUARD, &false);
        }
    }
    /// Terminate lease billing
    pub fn terminate_lease_billing(
        env: Env,
        lease_id: u64,
        terminator: Address,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
        let mut active_leases: Map<u64, ActiveLease> = env.storage().instance()
            .get(&ACTIVE_LEASES)
            .unwrap_or(Map::new(&env));
        let mut lease: ActiveLease = active_leases.get(lease_id)
            .ok_or(ContractError::LeaseNotFound)?;
        
        // Validate terminator (lessor or admin)
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if terminator != lease.lessor && terminator != admin {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::Unauthorized);
        }
        terminator.require_auth();
        
        // Update lease status
        lease.status = String::from_str(&env, "terminated");
        lease.billing_enabled = false;
        active_leases.set(lease_id, lease);
        env.storage().instance().set(&ACTIVE_LEASES, &active_leases);
        
        Self::exit_reentrancy_guard(&env);
        Ok(())
    }

    /// Get rent treasury information
    pub fn get_rent_treasury(env: Env) -> Result<RentTreasury, ContractError> {
        env.storage().instance()
            .get(&RENT_TREASURY)
            .ok_or(ContractError::TreasuryInsufficient)
    }

    /// Get billing state
    pub fn get_billing_state(env: Env) -> BillingState {
        env.storage().instance()
            .get(&BILLING_STATE)
            .unwrap_or_else(|| BillingState {
                global_billing_enabled: true,
                emergency_pause: false,
                pause_reason: None,
                pause_timestamp: None,
                total_cycles_processed: 0,
                total_rent_collected: 0,
                last_process_time: 0,
            })
    }

    /// Toggle emergency pause (admin only)
    pub fn toggle_emergency_pause(
        env: Env,
        admin: Address,
        pause: bool,
        reason: Option<String>,
    ) -> Result<(), ContractError> {
        let contract_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if contract_admin != admin {
            return Err(ContractError::Unauthorized);
        }
        admin.require_auth();
        
        let mut billing_state: BillingState = env.storage().instance()
            .get(&BILLING_STATE)
            .unwrap_or_else(|| BillingState {
                global_billing_enabled: true,
                emergency_pause: false,
                pause_reason: None,
                pause_timestamp: None,
                total_cycles_processed: 0,
                total_rent_collected: 0,
                last_process_time: 0,
            });
        
        billing_state.emergency_pause = pause;
        if pause {
            billing_state.pause_reason = reason;
            billing_state.pause_timestamp = Some(env.ledger().timestamp());
        } else {
            billing_state.pause_reason = None;
            billing_state.pause_timestamp = None;
        }
        
        env.storage().instance().set(&BILLING_STATE, &billing_state);
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
                continuous_billing_enabled: true,
                min_billing_interval: 3600,
                max_billing_interval: 2592000,
                reentrancy_protection_enabled: true,
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
            .unwrap_or_else(|| "2.0.0".into_val(&env))
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
        
        // Calculate remaining rent due using rent_per_second
        let current_time = env.ledger().timestamp();
        let remaining_cycles = if current_time >= lease.end_time {
            0
        } else {
            let remaining_time = lease.end_time - current_time;
            (remaining_time + lease.billing_frequency - 1) / lease.billing_frequency
        };
        
        let remaining_rent = if lease.rent_per_second > 0 {
            let remaining_seconds = if current_time >= lease.end_time {
                0
            } else {
                lease.end_time - current_time
            };
            lease.rent_per_second * remaining_seconds as i128
        } else {
            remaining_cycles as i128 * lease.rent_amount
        };
        
        let current_arrears = lease.total_owed - lease.total_paid;
        
        Ok((remaining_rent, current_arrears))
    }

    /// Distribute funds from rent treasury to lessor
    pub fn distribute_rent_treasury(
        env: Env,
        admin: Address,
        lessor: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        Self::enter_reentrancy_guard(&env)?;
        
        let contract_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if contract_admin != admin {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::Unauthorized);
        }
        admin.require_auth();
        
        let mut treasury: RentTreasury = env.storage().instance()
            .get(&RENT_TREASURY)
            .ok_or(ContractError::TreasuryInsufficient)?;
        
        if amount > treasury.available_balance {
            Self::exit_reentrancy_guard(&env);
            return Err(ContractError::TreasuryInsufficient);
        }
        
        // Transfer to lessor
        let token_client = TokenClient::new(&env, &treasury.treasury_address);
        token_client.transfer(&treasury.treasury_address, &lessor, &amount);
        
        // Update treasury
        treasury.total_distributed += amount;
        treasury.available_balance -= amount;
        env.storage().instance().set(&RENT_TREASURY, &treasury);
        
        Self::exit_reentrancy_guard(&env);
        Ok(())
    }
}
