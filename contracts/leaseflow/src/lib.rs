#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, env, symbol, Address, Bytes, Symbol, Vec, Map, U256, i64, u64};

// Contract state key
const DATA_KEY: Symbol = symbol!("DATA");

// Lease states
#[derive(Copy, Clone, Debug, Eq, PartialEq, contracttype)]
pub enum LeaseState {
    Pending,
    Active,
    GracePeriod,
    EvictionPending,
    Closed,
}

// Error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InsufficientRentFunds = 1,
    LeaseNotFound = 2,
    InvalidStateTransition = 3,
    Unauthorized = 4,
    GracePeriodExpired = 5,
    InsufficientDeposit = 6,
    InvalidAmount = 7,
    LeaseAlreadyActive = 8,
    LeaseNotActive = 9,
    EvictionAlreadyPending = 10,
    LateFeeCalculationError = 11,
}

// Events
#[contracttype]
pub struct RentDelinquencyStartedEvent {
    pub lease_id: u64,
    pub dunning_start_timestamp: u64,
    pub grace_period_end_timestamp: u64,
    pub outstanding_amount: i64,
    pub late_fee_amount: i64,
}

#[contracttype]
pub struct LeaseRecoveredEvent {
    pub lease_id: u64,
    pub recovery_timestamp: u64,
    pub total_paid: i64,
    pub late_fee_paid: i64,
}

#[contracttype]
pub struct LeaseCreatedEvent {
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub rent_amount: i64,
    pub deposit_amount: i64,
    pub start_date: u64,
    pub end_date: u64,
    pub max_grace_period: u64,
    pub late_fee_rate: u32, // basis points (10000 = 100%)
}

#[contracttype]
pub struct LeaseActivatedEvent {
    pub lease_id: u64,
    pub activation_timestamp: u64,
}

#[contracttype]
pub struct EvictionPendingEvent {
    pub lease_id: u64,
    pub eviction_timestamp: u64,
    pub total_outstanding: i64,
}

// Lease structure
#[derive(Clone, Debug, contracttype)]
pub struct Lease {
    pub lease_id: u64,
    pub lessor: Address,
    pub lessee: Address,
    pub rent_amount: i64,
    pub deposit_amount: i64,
    pub start_date: u64,
    pub end_date: u64,
    pub state: LeaseState,
    pub max_grace_period: u64, // in seconds, default 5 days = 432000 seconds
    pub late_fee_rate: u32,    // basis points (10000 = 100%)
    pub dunning_start_timestamp: Option<u64>,
    pub outstanding_balance: i64,
    pub accumulated_late_fees: i64,
    pub last_rent_payment_timestamp: u64,
    pub property_uri: Bytes,
}

// Contract data structure
#[derive(Clone, Debug, contracttype)]
pub struct ContractData {
    pub leases: Map<u64, Lease>,
    pub next_lease_id: u64,
}

// Contract implementation
#[contract]
pub struct LeaseFlowContract;

#[contractimpl]
impl LeaseFlowContract {
    // Initialize contract
    pub fn initialize(env: env::Env) {
        let data = ContractData {
            leases: Map::new(&env),
            next_lease_id: 1,
        };
        env.storage().instance().set(&DATA_KEY, &data);
    }

    // Create a new lease
    pub fn create_lease(
        env: env::Env,
        lessor: Address,
        lessee: Address,
        rent_amount: i64,
        deposit_amount: i64,
        start_date: u64,
        end_date: u64,
        max_grace_period: u64,
        late_fee_rate: u32,
        property_uri: Bytes,
    ) -> Result<u64, Error> {
        if rent_amount <= 0 || deposit_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if start_date >= end_date {
            return Err(Error::InvalidAmount);
        }

        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let lease_id = data.next_lease_id;

        let lease = Lease {
            lease_id,
            lessor: lessor.clone(),
            lessee: lessee.clone(),
            rent_amount,
            deposit_amount,
            start_date,
            end_date,
            state: LeaseState::Pending,
            max_grace_period,
            late_fee_rate,
            dunning_start_timestamp: None,
            outstanding_balance: 0,
            accumulated_late_fees: 0,
            last_rent_payment_timestamp: 0,
            property_uri,
        };

        data.leases.set(lease_id, lease);
        data.next_lease_id += 1;
        env.storage().instance().set(&DATA_KEY, &data);

        // Emit event
        env.events().publish(
            symbol!("LeaseCreated"),
            LeaseCreatedEvent {
                lease_id,
                lessor,
                lessee,
                rent_amount,
                deposit_amount,
                start_date,
                end_date,
                max_grace_period,
                late_fee_rate,
            },
        );

        Ok(lease_id)
    }

    // Activate lease (lessee deposits security deposit)
    pub fn activate_lease(env: env::Env, lease_id: u64, lessee: Address) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        // Verify caller is lessee
        if lessee != lease.lessee {
            return Err(Error::Unauthorized);
        }

        // Check state
        if lease.state != LeaseState::Pending {
            return Err(Error::LeaseAlreadyActive);
        }

        // In a real implementation, we would transfer tokens here
        // For now, we'll assume the deposit is available and update state
        
        lease.state = LeaseState::Active;
        lease.last_rent_payment_timestamp = env.ledger().timestamp();

        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);

        // Emit event
        env.events().publish(
            symbol!("LeaseActivated"),
            LeaseActivatedEvent {
                lease_id,
                activation_timestamp: env.ledger().timestamp(),
            },
        );

        Ok(())
    }

    // Process rent payment (called by payment stream or manual payment)
    pub fn process_rent_payment(env: env::Env, lease_id: u64, amount: i64) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        if amount < lease.rent_amount {
            return Err(Error::InsufficientRentFunds);
        }

        match lease.state {
            LeaseState::Active => {
                // Normal rent payment
                lease.last_rent_payment_timestamp = env.ledger().timestamp();
                lease.outstanding_balance = 0;
                lease.accumulated_late_fees = 0;
            }
            LeaseState::GracePeriod => {
                // Recovery payment during grace period
                let total_required = lease.rent_amount + lease.accumulated_late_fees;
                if amount < total_required {
                    return Err(Error::InsufficientRentFunds);
                }

                // Lease recovered
                lease.state = LeaseState::Active;
                lease.last_rent_payment_timestamp = env.ledger().timestamp();
                lease.outstanding_balance = 0;
                lease.accumulated_late_fees = 0;
                lease.dunning_start_timestamp = None;

                // Emit recovery event
                env.events().publish(
                    symbol!("LeaseRecovered"),
                    LeaseRecoveredEvent {
                        lease_id,
                        recovery_timestamp: env.ledger().timestamp(),
                        total_paid: amount,
                        late_fee_paid: lease.accumulated_late_fees,
                    },
                );
            }
            _ => return Err(Error::LeaseNotActive),
        }

        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);

        Ok(())
    }

    // Handle rent payment failure - trigger grace period
    pub fn handle_rent_payment_failure(env: env::Env, lease_id: u64) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        // Only trigger grace period from Active state
        if lease.state != LeaseState::Active {
            return Err(Error::InvalidStateTransition);
        }

        let current_timestamp = env.ledger().timestamp();
        
        // Transition to Grace Period
        lease.state = LeaseState::GracePeriod;
        lease.dunning_start_timestamp = Some(current_timestamp);
        lease.outstanding_balance = lease.rent_amount;

        // Calculate late fee
        let late_fee_amount = Self::calculate_late_fee(&lease, lease.rent_amount)?;
        lease.accumulated_late_fees = late_fee_amount;

        let grace_period_end = current_timestamp + lease.max_grace_period;

        // Emit delinquency event
        env.events().publish(
            symbol!("RentDelinquencyStarted"),
            RentDelinquencyStartedEvent {
                lease_id,
                dunning_start_timestamp: current_timestamp,
                grace_period_end,
                outstanding_amount: lease.outstanding_balance,
                late_fee_amount,
            },
        );

        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);

        Ok(())
    }

    // Check and handle grace period expiry
    pub fn check_grace_period_expiry(env: env::Env, lease_id: u64) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        if lease.state != LeaseState::GracePeriod {
            return Ok(()); // No action needed
        }

        let dunning_start = lease.dunning_start_timestamp.ok_or(Error::InvalidStateTransition)?;
        let current_timestamp = env.ledger().timestamp();

        if current_timestamp > dunning_start + lease.max_grace_period {
            // Grace period expired - transition to Eviction Pending
            lease.state = LeaseState::EvictionPending;
            
            let total_outstanding = lease.outstanding_balance + lease.accumulated_late_fees;

            // Emit eviction event
            env.events().publish(
                symbol!("EvictionPending"),
                EvictionPendingEvent {
                    lease_id,
                    eviction_timestamp: current_timestamp,
                    total_outstanding,
                },
            );

            data.leases.set(lease_id, lease);
            env.storage().instance().set(&DATA_KEY, &data);
        }

        Ok(())
    }

    // Calculate late fee based on rate and amount
    fn calculate_late_fee(lease: &Lease, base_amount: i64) -> Result<i64, Error> {
        // Convert basis points to multiplier (10000 = 100%)
        let fee_multiplier = U256::from_u32(lease.late_fee_rate);
        let basis_points = U256::from_u32(10000);
        
        // Calculate: base_amount * (late_fee_rate / 10000)
        let fee_amount = U256::from_i64(base_amount)
            .checked_mul(fee_multiplier)
            .and_then(|x| x.checked_div(basis_points))
            .ok_or(Error::LateFeeCalculationError)?;

        fee_amount.try_into().map_err(|_| Error::LateFeeCalculationError)
    }

    // Get lease information
    pub fn get_lease(env: env::Env, lease_id: u64) -> Result<Lease, Error> {
        let data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        data.leases.get(lease_id).ok_or(Error::LeaseNotFound)
    }

    // Get all leases for a specific address (either lessor or lessee)
    pub fn get_user_leases(env: env::Env, user: Address) -> Vec<u64> {
        let data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut user_leases = Vec::new(&env);

        for (lease_id, lease) in data.leases {
            if lease.lessor == user || lease.lessee == user {
                user_leases.push_back(lease_id);
            }
        }

        user_leases
    }

    // Emergency function to manually trigger grace period check
    pub fn trigger_grace_period_check(env: env::Env, lease_id: u64, caller: Address) -> Result<(), Error> {
        let data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        // Only lessor can trigger this check
        if caller != lease.lessor {
            return Err(Error::Unauthorized);
        }

        Self::check_grace_period_expiry(env, lease_id)
    }
}
