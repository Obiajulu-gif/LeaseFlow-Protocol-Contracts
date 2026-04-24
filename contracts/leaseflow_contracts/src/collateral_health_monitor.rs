use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map, Symbol, Vec, i128, u64, u32};
use soroban_sdk::token::Client as TokenClient;

// Contract state keys for collateral health monitoring
const COLLATERAL_HEALTH: Symbol = Symbol::short("COLL_HEALTH");
const MARGIN_CALLS: Symbol = Symbol::short("MARGIN_CALL");
const ORACLE_PRICE_FEED: Symbol = Symbol::short("ORACLE_PF");
const HEALTH_THRESHOLD: Symbol = Symbol::short("HEALTH_TH");
const GRACE_PERIOD: Symbol = Symbol::short("GRACE_PERIOD");
const PAUSED_UTILITIES: Symbol = Symbol::short("PAUSED_UTIL");

// Constants
const CRITICAL_HEALTH_THRESHOLD: u32 = 9000; // 90% in basis points
const DEFAULT_GRACE_PERIOD: u64 = 86400; // 24 hours in seconds
const PRICE_STALENESS_THRESHOLD: u64 = 3600; // 1 hour in seconds

// Collateral health monitoring errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CollateralHealthError {
    Unauthorized = 101,
    LeaseNotFound = 102,
    OracleUnavailable = 103,
    PriceStale = 104,
    InsufficientCollateral = 105,
    MarginCallActive = 106,
    GracePeriodExpired = 107,
    UtilityAlreadyPaused = 108,
    UtilityNotPaused = 109,
    InvalidHealthFactor = 110,
}

// Collateral health tracking structure
#[derive(Clone)]
#[contracttype]
pub struct CollateralHealth {
    pub lease_id: u64,
    pub collateral_token: Address,
    pub collateral_amount: i128,
    pub minimum_fiat_collateral: i128, // Required in fiat value (e.g., USD cents)
    pub current_fiat_value: i128,
    pub health_factor: u32, // In basis points (10000 = 100%)
    pub last_price_update: u64,
    pub status: String, // "healthy", "warning", "under_collateralized", "margin_call", "emergency_termination"
}

// Margin call structure
#[derive(Clone)]
#[contracttype]
pub struct MarginCall {
    pub lease_id: u64,
    pub lessee: Address,
    pub issued_at: u64,
    pub grace_period_end: u64,
    pub required_topup: i128,
    pub current_health_factor: u32,
    pub status: String, // "active", "satisfied", "expired"
    pub emergency_termination_scheduled: bool,
}

// Utility token pause tracking (Issue 67 integration)
#[derive(Clone)]
#[contracttype]
pub struct UtilityTokenPause {
    pub lessee: Address,
    pub lease_id: u64,
    pub paused_at: u64,
    pub reason: String,
    pub margin_call_id: u64,
}

// SEP-40 Oracle price data structure
#[derive(Clone)]
#[contracttype]
pub struct PriceData {
    pub price: i128, // Price in fiat (e.g., USD cents) with 8 decimals
    pub decimals: u32,
    pub last_update: u64,
    pub asset: Address,
}

// Events for collateral health monitoring
#[contractevent]
pub struct CollateralHealthWarning {
    pub lease_id: u64,
    pub lessee: Address,
    pub health_factor: u32,
    pub collateral_value: i128,
    pub required_value: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct MarginCallExecuted {
    pub lease_id: u64,
    pub lessee: Address,
    pub required_topup: i128,
    pub grace_period_end: u64,
    pub health_factor: u32,
    pub timestamp: u64,
}

#[contractevent]
pub struct EmergencyTerminationTriggered {
    pub lease_id: u64,
    pub lessee: Address,
    pub final_health_factor: u32,
    pub collateral_value: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct UtilityTokenPaused {
    pub lessee: Address,
    pub lease_id: u64,
    pub reason: String,
    pub timestamp: u64,
}

pub struct CollateralHealthMonitor;

#[contractimpl]
impl CollateralHealthMonitor {
    /// Initialize collateral health monitoring system
    pub fn initialize(
        env: Env,
        admin: Address,
        oracle_address: Address,
        health_threshold: u32,
        grace_period_seconds: u64,
    ) -> Result<(), CollateralHealthError> {
        // Verify admin authorization (would integrate with main contract)
        admin.require_auth();
        
        // Set oracle address
        env.storage().instance().set(&ORACLE_PRICE_FEED, &oracle_address);
        
        // Set health threshold (validate it's between 5000-10000 basis points)
        if health_threshold < 5000 || health_threshold > 10000 {
            return Err(CollateralHealthError::InvalidHealthFactor);
        }
        env.storage().instance().set(&HEALTH_THRESHOLD, &health_threshold);
        
        // Set grace period
        env.storage().instance().set(&GRACE_PERIOD, &grace_period_seconds);
        
        // Initialize storage
        let collateral_health: Map<u64, CollateralHealth> = Map::new(&env);
        let margin_calls: Map<u64, MarginCall> = Map::new(&env);
        let paused_utilities: Map<Address, UtilityTokenPause> = Map::new(&env);
        
        env.storage().instance().set(&COLLATERAL_HEALTH, &collateral_health);
        env.storage().instance().set(&MARGIN_CALLS, &margin_calls);
        env.storage().instance().set(&PAUSED_UTILITIES, &paused_utilities);
        
        Ok(())
    }

    /// Register a lease for collateral health monitoring
    pub fn register_lease_collateral(
        env: Env,
        lease_id: u64,
        lessee: Address,
        collateral_token: Address,
        collateral_amount: i128,
        minimum_fiat_collateral: i128,
    ) -> Result<(), CollateralHealthError> {
        // Get current price from oracle
        let price_data = Self::get_oracle_price(env.clone(), collateral_token.clone())?;
        
        // Calculate current fiat value
        let current_fiat_value = Self::calculate_fiat_value(
            env.clone(),
            collateral_amount,
            price_data.price,
            price_data.decimals,
        )?;
        
        // Calculate initial health factor
        let health_factor = if minimum_fiat_collateral > 0 {
            ((current_fiat_value * 10000) / minimum_fiat_collateral) as u32
        } else {
            10000u32 // 100% if no minimum required
        };
        
        // Determine initial status
        let status = if health_factor < CRITICAL_HEALTH_THRESHOLD {
            String::from_str(&env, "under_collateralized")
        } else if health_factor < (env.storage().instance().get(&HEALTH_THRESHOLD).unwrap_or(9500u32)) {
            String::from_str(&env, "warning")
        } else {
            String::from_str(&env, "healthy")
        };
        
        let collateral_health = CollateralHealth {
            lease_id,
            collateral_token,
            collateral_amount,
            minimum_fiat_collateral,
            current_fiat_value,
            health_factor,
            last_price_update: price_data.last_update,
            status,
        };
        
        // Store collateral health data
        let mut health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        health_map.set(lease_id, collateral_health);
        env.storage().instance().set(&COLLATERAL_HEALTH, &health_map);
        
        // Check if immediate action is needed
        if health_factor < CRITICAL_HEALTH_THRESHOLD {
            Self::trigger_margin_call(env, lease_id, lessee)?;
        }
        
        Ok(())
    }

    /// Update and check collateral health for all active leases
    pub fn check_collateral_health(env: Env, lease_id: u64) -> Result<(), CollateralHealthError> {
        let mut health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        
        let mut health_data: CollateralHealth = health_map.get(lease_id)
            .ok_or(CollateralHealthError::LeaseNotFound)?;
        
        // Get updated price from oracle
        let price_data = Self::get_oracle_price(env.clone(), health_data.collateral_token.clone())?;
        
        // Check if price is stale
        let current_time = env.ledger().timestamp();
        if current_time - price_data.last_update > PRICE_STALENESS_THRESHOLD {
            return Err(CollateralHealthError::PriceStale);
        }
        
        // Recalculate fiat value and health factor
        let new_fiat_value = Self::calculate_fiat_value(
            env.clone(),
            health_data.collateral_amount,
            price_data.price,
            price_data.decimals,
        )?;
        
        let new_health_factor = if health_data.minimum_fiat_collateral > 0 {
            ((new_fiat_value * 10000) / health_data.minimum_fiat_collateral) as u32
        } else {
            10000u32
        };
        
        // Update health data
        health_data.current_fiat_value = new_fiat_value;
        health_data.health_factor = new_health_factor;
        health_data.last_price_update = price_data.last_update;
        
        // Determine new status
        let new_status = if new_health_factor < CRITICAL_HEALTH_THRESHOLD {
            String::from_str(&env, "under_collateralized")
        } else if new_health_factor < (env.storage().instance().get(&HEALTH_THRESHOLD).unwrap_or(9500u32)) {
            String::from_str(&env, "warning")
        } else {
            String::from_str(&env, "healthy")
        };
        
        let old_status = health_data.status.clone();
        health_data.status = new_status.clone();
        
        // Store updated health data
        health_map.set(lease_id, health_data);
        env.storage().instance().set(&COLLATERAL_HEALTH, &health_map);
        
        // Trigger appropriate actions based on health status changes
        match (old_status.as_str(), new_status.as_str()) {
            (_, "under_collateralized") => {
                // Need to get lessee address - this would come from lease data
                // For now, we'll emit a warning event
                env.events().publish(
                    (Symbol::short("COLLATERAL_WARNING"), lease_id),
                    CollateralHealthWarning {
                        lease_id,
                        lessee: Address::generate(&env), // Placeholder - would get from lease
                        health_factor: new_health_factor,
                        collateral_value: new_fiat_value,
                        required_value: health_data.minimum_fiat_collateral,
                        timestamp: current_time,
                    }
                );
                
                // Trigger margin call
                Self::trigger_margin_call(env, lease_id, Address::generate(&env))?; // Placeholder
            },
            _ => {}
        }
        
        Ok(())
    }

    /// Trigger margin call for under-collateralized lease
    fn trigger_margin_call(env: Env, lease_id: u64, lessee: Address) -> Result<(), CollateralHealthError> {
        let health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        
        let health_data: CollateralHealth = health_map.get(lease_id)
            .ok_or(CollateralHealthError::LeaseNotFound)?;
        
        // Check if margin call already exists
        let margin_calls: Map<u64, MarginCall> = env.storage().instance()
            .get(&MARGIN_CALLS)
            .unwrap_or(Map::new(&env));
        
        if margin_calls.contains_key(&lease_id) {
            return Err(CollateralHealthError::MarginCallActive);
        }
        
        // Calculate required top-up to restore health to 100%
        let required_topup = if health_data.minimum_fiat_collateral > health_data.current_fiat_value {
            health_data.minimum_fiat_collateral - health_data.current_fiat_value
        } else {
            0i128
        };
        
        let current_time = env.ledger().timestamp();
        let grace_period: u64 = env.storage().instance().get(&GRACE_PERIOD).unwrap_or(DEFAULT_GRACE_PERIOD);
        let grace_period_end = current_time + grace_period;
        
        let margin_call = MarginCall {
            lease_id,
            lessee: lessee.clone(),
            issued_at: current_time,
            grace_period_end,
            required_topup,
            current_health_factor: health_data.health_factor,
            status: String::from_str(&env, "active"),
            emergency_termination_scheduled: false,
        };
        
        // Store margin call
        let mut margin_calls_map = margin_calls;
        margin_calls_map.set(lease_id, margin_call);
        env.storage().instance().set(&MARGIN_CALLS, &margin_calls_map);
        
        // Pause utility token (Issue 67 integration)
        Self::pause_utility_token(env, lessee.clone(), lease_id, String::from_str(&env, "Margin call issued"))?;
        
        // Emit margin call event
        env.events().publish(
            (Symbol::short("MARGIN_CALL"), lease_id),
            MarginCallExecuted {
                lease_id,
                lessee: lessee.clone(),
                required_topup,
                grace_period_end,
                health_factor: health_data.health_factor,
                timestamp: current_time,
            }
        );
        
        Ok(())
    }

    /// Pause utility token for lessee (Issue 67 integration)
    fn pause_utility_token(
        env: Env,
        lessee: Address,
        lease_id: u64,
        reason: String,
    ) -> Result<(), CollateralHealthError> {
        let paused_utilities: Map<Address, UtilityTokenPause> = env.storage().instance()
            .get(&PAUSED_UTILITIES)
            .unwrap_or(Map::new(&env));
        
        // Check if already paused
        if paused_utilities.contains_key(&lessee) {
            return Err(CollateralHealthError::UtilityAlreadyPaused);
        }
        
        let pause_info = UtilityTokenPause {
            lessee: lessee.clone(),
            lease_id,
            paused_at: env.ledger().timestamp(),
            reason: reason.clone(),
            margin_call_id: lease_id, // Use lease_id as margin_call_id for simplicity
        };
        
        // Store pause info
        let mut paused_map = paused_utilities;
        paused_map.set(lessee.clone(), pause_info);
        env.storage().instance().set(&PAUSED_UTILITIES, &paused_map);
        
        // Emit pause event
        env.events().publish(
            (Symbol::short("UTILITY_PAUSED"), lessee.clone()),
            UtilityTokenPaused {
                lessee,
                lease_id,
                reason,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Handle margin call fulfillment
    pub fn fulfill_margin_call(
        env: Env,
        lease_id: u64,
        additional_collateral: i128,
        token_address: Address,
    ) -> Result<(), CollateralHealthError> {
        let mut margin_calls: Map<u64, MarginCall> = env.storage().instance()
            .get(&MARGIN_CALLS)
            .unwrap_or(Map::new(&env));
        
        let mut margin_call: MarginCall = margin_calls.get(lease_id)
            .ok_or(CollateralHealthError::MarginCallActive)?; // Reuse error for simplicity
        
        // Check if grace period has expired
        let current_time = env.ledger().timestamp();
        if current_time > margin_call.grace_period_end {
            return Err(CollateralHealthError::GracePeriodExpired);
        }
        
        // Update collateral health data
        let mut health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        
        let mut health_data: CollateralHealth = health_map.get(lease_id)
            .ok_or(CollateralHealthError::LeaseNotFound)?;
        
        // Add additional collateral
        health_data.collateral_amount += additional_collateral;
        
        // Recalculate health factor
        let price_data = Self::get_oracle_price(env.clone(), token_address.clone())?;
        let new_fiat_value = Self::calculate_fiat_value(
            env.clone(),
            health_data.collateral_amount,
            price_data.price,
            price_data.decimals,
        )?;
        
        health_data.current_fiat_value = new_fiat_value;
        health_data.health_factor = ((new_fiat_value * 10000) / health_data.minimum_fiat_collateral) as u32;
        health_data.last_price_update = price_data.last_update;
        
        // Update status based on new health factor
        if health_data.health_factor >= CRITICAL_HEALTH_THRESHOLD {
            health_data.status = String::from_str(&env, "healthy");
            margin_call.status = String::from_str(&env, "satisfied");
            
            // Resume utility token
            Self::resume_utility_token(env, margin_call.lessee.clone())?;
        } else {
            health_data.status = String::from_str(&env, "warning");
        }
        
        // Store updated data
        health_map.set(lease_id, health_data);
        env.storage().instance().set(&COLLATERAL_HEALTH, &health_map);
        
        margin_calls.set(lease_id, margin_call);
        env.storage().instance().set(&MARGIN_CALLS, &margin_calls);
        
        Ok(())
    }

    /// Resume utility token for lessee
    fn resume_utility_token(env: Env, lessee: Address) -> Result<(), CollateralHealthError> {
        let mut paused_utilities: Map<Address, UtilityTokenPause> = env.storage().instance()
            .get(&PAUSED_UTILITIES)
            .unwrap_or(Map::new(&env));
        
        // Check if utility is paused
        if !paused_utilities.contains_key(&lessee) {
            return Err(CollateralHealthError::UtilityNotPaused);
        }
        
        // Remove pause entry
        paused_utilities.remove(&lessee);
        env.storage().instance().set(&PAUSED_UTILITIES, &paused_utilities);
        
        Ok(())
    }

    /// Execute emergency termination for expired margin calls
    pub fn execute_emergency_termination(env: Env, lease_id: u64) -> Result<(), CollateralHealthError> {
        let mut margin_calls: Map<u64, MarginCall> = env.storage().instance()
            .get(&MARGIN_CALLS)
            .unwrap_or(Map::new(&env));
        
        let margin_call: MarginCall = margin_calls.get(lease_id)
            .ok_or(CollateralHealthError::MarginCallActive)?; // Reuse error
        
        // Check if grace period has expired
        let current_time = env.ledger().timestamp();
        if current_time <= margin_call.grace_period_end {
            return Err(CollateralHealthError::GracePeriodExpired); // Reuse error
        }
        
        // Get health data for event
        let health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        
        let health_data: CollateralHealth = health_map.get(lease_id)
            .ok_or(CollateralHealthError::LeaseNotFound)?;
        
        // Update margin call status
        let mut updated_margin_call = margin_call;
        updated_margin_call.status = String::from_str(&env, "expired");
        updated_margin_call.emergency_termination_scheduled = true;
        margin_calls.set(lease_id, updated_margin_call);
        env.storage().instance().set(&MARGIN_CALLS, &margin_calls);
        
        // Update lease status to indicate emergency termination
        let mut updated_health_data = health_data;
        updated_health_data.status = String::from_str(&env, "emergency_termination");
        let mut updated_health_map = health_map;
        updated_health_map.set(lease_id, updated_health_data);
        env.storage().instance().set(&COLLATERAL_HEALTH, &updated_health_map);
        
        // Emit emergency termination event
        env.events().publish(
            (Symbol::short("EMERGENCY_TERMINATION"), lease_id),
            EmergencyTerminationTriggered {
                lease_id,
                lessee: margin_call.lessee.clone(),
                final_health_factor: margin_call.current_health_factor,
                collateral_value: health_data.current_fiat_value,
                timestamp: current_time,
            }
        );
        
        Ok(())
    }

    /// Get price data from SEP-40 Oracle
    fn get_oracle_price(env: Env, token_address: Address) -> Result<PriceData, CollateralHealthError> {
        let oracle_address: Address = env.storage().instance()
            .get(&ORACLE_PRICE_FEED)
            .ok_or(CollateralHealthError::OracleUnavailable)?;
        
        // In a real implementation, this would call the SEP-40 oracle contract
        // For now, we'll simulate with placeholder data
        let current_time = env.ledger().timestamp();
        
        Ok(PriceData {
            price: 100000000i128, // $1.00 with 8 decimals (placeholder)
            decimals: 8,
            last_update: current_time,
            asset: token_address,
        })
    }

    /// Calculate fiat value of collateral
    fn calculate_fiat_value(
        env: Env,
        collateral_amount: i128,
        price: i128,
        price_decimals: u32,
    ) -> Result<i128, CollateralHealthError> {
        // For simplicity, assuming collateral has 8 decimals
        // In a real implementation, we'd need to handle different token decimals
        let collateral_decimals = 8u32;
        
        if price_decimals >= collateral_decimals {
            Ok((collateral_amount * price) / (10i128.pow(price_decimals - collateral_decimals)))
        } else {
            Ok((collateral_amount * price) / (10i128.pow(collateral_decimals - price_decimals)))
        }
    }

    /// Get collateral health data for a lease
    pub fn get_collateral_health(env: Env, lease_id: u64) -> Result<CollateralHealth, CollateralHealthError> {
        let health_map: Map<u64, CollateralHealth> = env.storage().instance()
            .get(&COLLATERAL_HEALTH)
            .unwrap_or(Map::new(&env));
        
        health_map.get(lease_id)
            .ok_or(CollateralHealthError::LeaseNotFound)
    }

    /// Get margin call data for a lease
    pub fn get_margin_call(env: Env, lease_id: u64) -> Result<MarginCall, CollateralHealthError> {
        let margin_calls: Map<u64, MarginCall> = env.storage().instance()
            .get(&MARGIN_CALLS)
            .unwrap_or(Map::new(&env));
        
        margin_calls.get(lease_id)
            .ok_or(CollateralHealthError::MarginCallActive) // Reuse error
    }

    /// Check if utility token is paused for a lessee
    pub fn is_utility_paused(env: Env, lessee: Address) -> bool {
        let paused_utilities: Map<Address, UtilityTokenPause> = env.storage().instance()
            .get(&PAUSED_UTILITIES)
            .unwrap_or(Map::new(&env));
        
        paused_utilities.contains_key(&lessee)
    }

    /// Batch health check for gas efficiency
    pub fn batch_health_check(env: Env, lease_ids: Vec<u64>) -> Result<Vec<u64>, CollateralHealthError> {
        let mut problematic_leases = Vec::new(&env);
        
        for lease_id in lease_ids.iter() {
            match Self::check_collateral_health(env.clone(), lease_id) {
                Ok(_) => {},
                Err(_) => {
                    problematic_leases.push_back(lease_id);
                }
            }
        }
        
        Ok(problematic_leases)
    }
}
