#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, env, symbol, Address, Bytes, Symbol, Vec, Map, U256, i64, u64};
use leaseflow_math::{calculate_prorated_rent, calculate_termination_refund};

// SEP-40 Oracle interface
#[contracttype]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
    pub asset: Address,
    pub decimals: u32,
}

#[contracttype]
pub struct OracleConfig {
    pub oracle_address: Address,
    pub staleness_threshold: u64, // 15 minutes = 900 seconds
    pub volatility_threshold: u32, // 20% = 2000 basis points
    pub max_price_age: u64, // Maximum age for price data
}

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
    ArrearsAlreadyProcessed = 12,
    EscrowVaultUnderflow = 13,
    CreditRecordError = 14,
    OracleDataStale = 15,
    OraclePriceManipulation = 16,
    VolatilityCircuitBreaker = 17,
    OracleCallFailed = 18,
    InvalidFiatPegConfig = 19,
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

#[contracttype]
pub struct DepositSlashedForArrearsEvent {
    pub lease_id: u64,
    pub unpaid_duration: u64,
    pub deducted_amount: i64,
    pub remaining_escrow_balance: i64,
    pub residual_debt: i64,
}

#[contracttype]
pub struct ProratedRentCalculatedEvent {
    pub lease_id: u64,
    pub monthly_rent: i64,
    pub elapsed_seconds: u64,
    pub prorated_amount: i64,
    pub calculation_type: Symbol, // "initialization" or "termination"
}

#[contracttype]
pub struct FiatPeggedRentBilledEvent {
    pub lease_id: u64,
    pub target_usd_amount: i64,
    pub oracle_exchange_rate: i128,
    pub final_crypto_deduction: i64,
    pub billing_timestamp: u64,
    pub asset_address: Address,
}

// Protocol Credit Record for tracking residual debt
#[derive(Clone, Debug, contracttype)]
pub struct ProtocolCreditRecord {
    pub lessee: Address,
    pub total_debt_amount: i64,
    pub default_count: u32,
    pub last_default_timestamp: u64,
    pub associated_lease_ids: Vec<u64>,
}

// Escrow Vault structure for managing security deposits
#[derive(Clone, Debug, contracttype)]
pub struct EscrowVault {
    pub total_locked: i64,
    pub available_balance: i64,
    pub lessor_treasury: i64,
}

// Fiat peg configuration for dynamic pricing
#[derive(Clone, Debug, contracttype)]
pub struct FiatPegConfig {
    pub enabled: bool,
    pub target_usd_amount: i64, // Target USD amount per billing cycle
    pub asset_address: Address, // Crypto asset address (e.g., XLM)
    pub oracle_address: Address, // SEP-40 Oracle address
    pub staleness_threshold: u64, // Seconds before data is considered stale
    pub volatility_threshold: u32, // Basis points for volatility circuit breaker
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
    pub arrears_processed: bool, // Track if arrears deduction has been executed
    pub prorated_initial_rent: i64, // Track prorated rent for initial partial period
    pub total_paid_rent: i64, // Track total rent paid for refund calculations
    pub fiat_peg_config: Option<FiatPegConfig>, // Dynamic fiat peg configuration
    pub last_oracle_price: Option<i128>, // Cache last oracle price for volatility detection
    pub last_oracle_timestamp: Option<u64>, // Cache last oracle timestamp for staleness check
}

// Contract data structure
#[derive(Clone, Debug, contracttype)]
pub struct ContractData {
    pub leases: Map<u64, Lease>,
    pub next_lease_id: u64,
    pub escrow_vault: EscrowVault,
    pub credit_records: Map<Address, ProtocolCreditRecord>,
    pub oracle_config: OracleConfig, // Global oracle configuration
}

// Contract implementation
#[contract]
pub struct LeaseFlowContract;

#[contractimpl]
impl LeaseFlowContract {
    // Initialize contract
    pub fn initialize(env: env::Env, oracle_address: Address) {
        let escrow_vault = EscrowVault {
            total_locked: 0,
            available_balance: 0,
            lessor_treasury: 0,
        };
        
        let oracle_config = OracleConfig {
            oracle_address: oracle_address.clone(),
            staleness_threshold: 900, // 15 minutes
            volatility_threshold: 2000, // 20% in basis points
            max_price_age: 3600, // 1 hour maximum age
        };
        
        let data = ContractData {
            leases: Map::new(&env),
            next_lease_id: 1,
            escrow_vault,
            credit_records: Map::new(&env),
            oracle_config,
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
        fiat_peg_config: Option<FiatPegConfig>,
    ) -> Result<u64, Error> {
        if rent_amount <= 0 || deposit_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if start_date >= end_date {
            return Err(Error::InvalidAmount);
        }

        // Validate fiat peg configuration if provided
        if let Some(ref config) = fiat_peg_config {
            if config.target_usd_amount <= 0 {
                return Err(Error::InvalidFiatPegConfig);
            }
            if config.staleness_threshold == 0 || config.volatility_threshold == 0 {
                return Err(Error::InvalidFiatPegConfig);
            }
        }

        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let lease_id = data.next_lease_id;

        // Calculate prorated rent for initial period
        let current_timestamp = env.ledger().timestamp();
        let prorated_initial_rent = if current_timestamp > start_date {
            // Lease starts mid-cycle - calculate prorated rent for remaining period
            calculate_prorated_rent(rent_amount, current_timestamp, end_date)
                .map(|(amount, _)| amount)
                .unwrap_or(rent_amount) // Fallback to full rent if calculation fails
        } else {
            rent_amount // Full rent for future start dates
        };

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
            arrears_processed: false,
            prorated_initial_rent,
            total_paid_rent: 0,
            fiat_peg_config,
            last_oracle_price: None,
            last_oracle_timestamp: None,
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
        
        // Update escrow vault with the security deposit
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        data.escrow_vault.total_locked += deposit_amount;
        data.escrow_vault.available_balance += deposit_amount;
        
        lease.state = LeaseState::Active;
        lease.last_rent_payment_timestamp = env.ledger().timestamp();
        
        // Emit ProratedRentCalculated event if prorated rent was applied
        if lease.prorated_initial_rent != lease.rent_amount {
            let elapsed_seconds = lease.end_date.saturating_sub(env.ledger().timestamp());
            env.events().publish(
                symbol!("ProratedRentCalculated"),
                ProratedRentCalculatedEvent {
                    lease_id,
                    monthly_rent: lease.rent_amount,
                    elapsed_seconds,
                    prorated_amount: lease.prorated_initial_rent,
                    calculation_type: symbol!("initialization"),
                },
            );
        }

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
                lease.total_paid_rent += amount;
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
                lease.total_paid_rent += amount;

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
            
            // Automatically execute arrears deduction
            Self::execute_arrears_deduction(env, lease_id)?;
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

    // Execute automated security deposit deduction for rent arrears
    pub fn execute_arrears_deduction(env: env::Env, lease_id: u64) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        // Only execute from EvictionPending state and ensure not already processed
        if lease.state != LeaseState::EvictionPending {
            return Err(Error::InvalidStateTransition);
        }
        
        if lease.arrears_processed {
            return Err(Error::ArrearsAlreadyProcessed);
        }

        let current_timestamp = env.ledger().timestamp();
        
        // Calculate unpaid duration (from dunning start to eviction)
        let dunning_start = lease.dunning_start_timestamp.ok_or(Error::InvalidStateTransition)?;
        let unpaid_duration = current_timestamp.saturating_sub(dunning_start);

        // Calculate total arrears (unpaid rent + late fees)
        let total_arrears = lease.outstanding_balance + lease.accumulated_late_fees;

        // Calculate deduction amount with safety rounding in favor of protocol
        let deduction_amount = Self::calculate_deduction_amount(total_arrears, lease.deposit_amount)?;

        // Update escrow vault - transfer to lessor's operational treasury
        if data.escrow_vault.available_balance < deduction_amount {
            return Err(Error::EscrowVaultUnderflow);
        }
        
        data.escrow_vault.available_balance -= deduction_amount;
        data.escrow_vault.lessor_treasury += deduction_amount;

        // Calculate residual debt (if any)
        let residual_debt = total_arrears.saturating_sub(deduction_amount);

        // Update lease to mark arrears as processed
        lease.arrears_processed = true;

        // Handle residual debt tracking
        if residual_debt > 0 {
            Self::update_credit_record(&env, &mut data, lease.lessee.clone(), residual_debt, lease_id)?;
        }

        // Emit detailed event
        env.events().publish(
            symbol!("DepositSlashedForArrears"),
            DepositSlashedForArrearsEvent {
                lease_id,
                unpaid_duration,
                deducted_amount: deduction_amount,
                remaining_escrow_balance: data.escrow_vault.available_balance,
                residual_debt,
            },
        );

        // Save updated data
        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);

        Ok(())
    }

    // Calculate deduction amount with safety rounding in favor of protocol
    fn calculate_deduction_amount(total_arrears: i64, deposit_amount: i64) -> Result<i64, Error> {
        if total_arrears <= 0 || deposit_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // If arrears exceed deposit, drain entire deposit
        if total_arrears >= deposit_amount {
            return Ok(deposit_amount);
        }

        // Otherwise, deduct exact arrears amount
        Ok(total_arrears)
    }

    // Update or create credit record for residual debt
    fn update_credit_record(
        env: &env::Env,
        data: &mut ContractData, 
        lessee: Address, 
        residual_debt: i64, 
        lease_id: u64
    ) -> Result<(), Error> {
        let current_timestamp = env.ledger().timestamp();
        
        let mut record = data.credit_records.get(&lessee).unwrap_or_else(|| ProtocolCreditRecord {
            lessee: lessee.clone(),
            total_debt_amount: 0,
            default_count: 0,
            last_default_timestamp: 0,
            associated_lease_ids: Vec::new(env),
        });

        // Update record with new debt
        record.total_debt_amount += residual_debt;
        record.default_count += 1;
        record.last_default_timestamp = current_timestamp;
        
        // Add lease ID if not already present
        if !record.associated_lease_ids.contains(&lease_id) {
            record.associated_lease_ids.push_back(lease_id);
        }

        data.credit_records.set(lessee, record);
        Ok(())
    }

    // Get credit record for a specific lessee
    pub fn get_credit_record(env: env::Env, lessee: Address) -> Result<ProtocolCreditRecord, Error> {
        let data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        data.credit_records.get(&lessee).ok_or(Error::CreditRecordError)
    }

    // Get escrow vault information
    pub fn get_escrow_vault(env: env::Env) -> Result<EscrowVault, Error> {
        let data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        Ok(data.escrow_vault)
    }

    // Fetch price data from SEP-40 Oracle with security checks
    fn get_oracle_price(env: &env::Env, lease: &Lease, oracle_address: &Address) -> Result<PriceData, Error> {
        let current_timestamp = env.ledger().timestamp();
        
        // Call SEP-40 Oracle to get price data
        let oracle_client = soroban_sdk::contractclient::ContractClient::new(env, oracle_address);
        let price_data: PriceData = oracle_client.invoke(
            &symbol!("get_price"),
            &lease.fiat_peg_config.as_ref().unwrap().asset_address,
        ).try_into().map_err(|_| Error::OracleCallFailed)?;
        
        // Staleness check
        if current_timestamp.saturating_sub(price_data.timestamp) > lease.fiat_peg_config.as_ref().unwrap().staleness_threshold {
            return Err(Error::OracleDataStale);
        }
        
        // Additional security: ensure price is not too old
        if current_timestamp.saturating_sub(price_data.timestamp) > 3600 { // 1 hour max
            return Err(Error::OracleDataStale);
        }
        
        // Flash loan attack protection: check for extreme price changes
        if let (Some(last_price), Some(last_timestamp)) = (lease.last_oracle_price, lease.last_oracle_timestamp) {
            let time_diff = current_timestamp.saturating_sub(last_timestamp);
            
            // Only check volatility if we have recent data (within last hour)
            if time_diff <= 3600 && last_price > 0 {
                let price_change_percent = if price_data.price > last_price {
                    ((price_data.price - last_price) * 10000) / last_price
                } else {
                    ((last_price - price_data.price) * 10000) / last_price
                };
                
                // If price change exceeds threshold, trigger circuit breaker
                if price_change_percent > lease.fiat_peg_config.as_ref().unwrap().volatility_threshold as i128 {
                    return Err(Error::VolatilityCircuitBreaker);
                }
            }
        }
        
        Ok(price_data)
    }

    // Calculate fiat-pegged rent amount
    fn calculate_fiat_pegged_rent(env: &env::Env, lease: &Lease) -> Result<i64, Error> {
        let config = lease.fiat_peg_config.as_ref().ok_or(Error::InvalidFiatPegConfig)?;
        
        if !config.enabled {
            return Ok(lease.rent_amount); // Return fixed amount if not enabled
        }
        
        // Get current price from oracle
        let price_data = Self::get_oracle_price(env, lease, &config.oracle_address)?;
        
        // Convert USD target to crypto amount
        // Formula: crypto_amount = (target_usd * 10^decimals) / price
        let decimals_factor = 10i128.pow(price_data.decimals);
        let usd_target_i128 = config.target_usd_amount as i128;
        
        let crypto_amount_i128 = (usd_target_i128 * decimals_factor) / price_data.price;
        
        // Convert to i64 with safety check
        let crypto_amount = if crypto_amount_i128 > i64::MAX as i128 {
            return Err(Error::InvalidAmount);
        } else {
            crypto_amount_i128 as i64
        };
        
        // Ensure minimum rent amount
        if crypto_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        
        Ok(crypto_amount)
    }

    // Process fiat-pegged rent payment
    pub fn process_fiat_pegged_rent_payment(env: env::Env, lease_id: u64) -> Result<(), Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;
        
        // Only process for active leases with fiat peg enabled
        if lease.state != LeaseState::Active {
            return Err(Error::LeaseNotActive);
        }
        
        let config = lease.fiat_peg_config.as_ref().ok_or(Error::InvalidFiatPegConfig)?;
        if !config.enabled {
            return Err(Error::InvalidFiatPegConfig);
        }
        
        // Calculate current rent amount based on oracle
        let current_rent_amount = Self::calculate_fiat_pegged_rent(&env, &lease)?;
        
        // Get oracle price data for event emission
        let price_data = Self::get_oracle_price(&env, &lease, &config.oracle_address)?;
        
        // Update lease with new oracle data for next volatility check
        lease.last_oracle_price = Some(price_data.price);
        lease.last_oracle_timestamp = Some(price_data.timestamp);
        lease.last_rent_payment_timestamp = env.ledger().timestamp();
        lease.total_paid_rent += current_rent_amount;
        
        // Emit FiatPeggedRentBilled event
        env.events().publish(
            symbol!("FiatPeggedRentBilled"),
            FiatPeggedRentBilledEvent {
                lease_id,
                target_usd_amount: config.target_usd_amount,
                oracle_exchange_rate: price_data.price,
                final_crypto_deduction: current_rent_amount,
                billing_timestamp: env.ledger().timestamp(),
                asset_address: config.asset_address.clone(),
            },
        );
        
        // Save updated lease
        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);
        
        Ok(())
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

    // Terminate lease amicably with prorated refund
    pub fn terminate_lease(env: env::Env, lease_id: u64, caller: Address) -> Result<i64, Error> {
        let mut data: ContractData = env.storage().instance().get(&DATA_KEY).unwrap();
        let mut lease = data.leases.get(lease_id).ok_or(Error::LeaseNotFound)?;

        // Only lessor or lessee can terminate
        if caller != lease.lessor && caller != lease.lessee {
            return Err(Error::Unauthorized);
        }

        // Can only terminate active leases
        if lease.state != LeaseState::Active {
            return Err(Error::LeaseNotActive);
        }

        let current_timestamp = env.ledger().timestamp();
        
        // Calculate prorated refund for unused period
        let refund_amount = calculate_termination_refund(
            lease.rent_amount,
            lease.start_date,
            lease.end_date,
            current_timestamp,
            lease.total_paid_rent,
        ).unwrap_or(0);

        // Apply security measure: ensure minimum holding period to prevent exploitation
        let minimum_holding_period = 86400; // 24 hours minimum
        let lease_duration = current_timestamp.saturating_sub(lease.last_rent_payment_timestamp);
        
        if lease_duration < minimum_holding_period && refund_amount > 0 {
            // Apply penalty for rapid termination to prevent exploitation
            let penalty_amount = refund_amount / 10; // 10% penalty
            let adjusted_refund = refund_amount.saturating_sub(penalty_amount);
            
            // Emit ProratedRentCalculated event with penalty
            env.events().publish(
                symbol!("ProratedRentCalculated"),
                ProratedRentCalculatedEvent {
                    lease_id,
                    monthly_rent: lease.rent_amount,
                    elapsed_seconds: lease.end_date.saturating_sub(current_timestamp),
                    prorated_amount: adjusted_refund,
                    calculation_type: symbol!("termination"),
                },
            );

            // Update lease state
            lease.state = LeaseState::Closed;
            data.leases.set(lease_id, lease);
            env.storage().instance().set(&DATA_KEY, &data);

            return Ok(adjusted_refund);
        }

        // Emit ProratedRentCalculated event
        env.events().publish(
            symbol!("ProratedRentCalculated"),
            ProratedRentCalculatedEvent {
                lease_id,
                monthly_rent: lease.rent_amount,
                elapsed_seconds: lease.end_date.saturating_sub(current_timestamp),
                prorated_amount: refund_amount,
                calculation_type: symbol!("termination"),
            },
        );

        // Update lease state
        lease.state = LeaseState::Closed;
        data.leases.set(lease_id, lease);
        env.storage().instance().set(&DATA_KEY, &data);

        Ok(refund_amount)
    }
}
