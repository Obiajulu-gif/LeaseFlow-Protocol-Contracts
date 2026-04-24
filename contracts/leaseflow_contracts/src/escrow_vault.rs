use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map, Symbol, Vec, i128, u64, u32};
use soroban_sdk::token::Client as TokenClient;

// Contract state keys
const ADMIN: Symbol = Symbol::short("ADMIN");
const DAO_SECURITY_COUNCIL: Symbol = Symbol::short("DAO_SEC");
const IS_ESCROW_FROZEN: Symbol = Symbol::short("FROZEN");
const ESCROW_ENTRIES: Symbol = Symbol::short("ESCROWS");
const FREEZE_TIMESTAMP: Symbol = Symbol::short("FREEZE_TS");
const CONTRACT_VERSION: Symbol = Symbol::short("VERSION");

// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    Unauthorized = 1,
    EscrowFrozen = 2,
    EscrowNotFound = 3,
    InvalidInput = 4,
    InsufficientFunds = 5,
    AlreadyReleased = 6,
    TransferFailed = 7,
    InvalidStatus = 8,
}

// Escrow entry structure
#[derive(Clone)]
#[contracttype]
pub struct EscrowEntry {
    pub escrow_id: u64,
    pub depositor: Address,
    pub beneficiary: Address,
    pub amount: i128,
    pub token: Address,
    pub purpose: String, // "security_deposit", "slash_amount", "mutual_release"
    pub status: String,  // "pending", "locked", "released", "refunded"
    pub created_at: u64,
    pub lock_until: u64,
    pub conditions_met: bool,
    pub release_signature: Option<BytesN<32>>,
    pub lease_id: Option<u64>,
}

// Events
#[contractevent]
pub struct EmergencyEscrowFreezeActivated {
    pub frozen_by: Address,
    pub timestamp: u64,
    pub reason: String,
}

#[contractevent]
pub struct EmergencyEscrowFreezeLifted {
    pub lifted_by: Address,
    pub frozen_duration: u64,
    pub timestamp: u64,
}

#[contractevent]
pub struct EscrowDepositInitialized {
    pub escrow_id: u64,
    pub depositor: Address,
    pub beneficiary: Address,
    pub amount: i128,
    pub token: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct EscrowOracleSlashExecuted {
    pub escrow_id: u64,
    pub slashed_amount: i128,
    pub slashed_by: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct EscrowMutualRelease {
    pub escrow_id: u64,
    pub released_to: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct EscrowArrearsDeducted {
    pub escrow_id: u64,
    pub deducted_amount: i128,
    pub deducted_to: Address,
    pub timestamp: u64,
}

pub struct EscrowVault;

#[contractimpl]
impl EscrowVault {
    /// Initialize the Escrow Vault contract
    pub fn initialize(env: Env, admin: Address, dao_security_council: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(ContractError::Unauthorized);
        }
        
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&DAO_SECURITY_COUNCIL, &dao_security_council);
        env.storage().instance().set(&CONTRACT_VERSION, &"1.0.0");
        
        // Initialize freeze state to false
        env.storage().instance().set(&IS_ESCROW_FROZEN, &false);
        
        // Initialize escrow storage
        let escrows: Map<u64, EscrowEntry> = Map::new(&env);
        env.storage().instance().set(&ESCROW_ENTRIES, &escrows);
        
        Ok(())
    }

    /// Initialize a new escrow deposit
    pub fn initialize_deposit(
        env: Env,
        depositor: Address,
        beneficiary: Address,
        token: Address,
        amount: i128,
        purpose: String,
        lock_duration: u64,
        lease_id: Option<u64>,
    ) -> Result<u64, ContractError> {
        // Check freeze status
        Self::check_freeze_status(&env)?;
        
        depositor.require_auth();
        
        let current_time = env.ledger().timestamp();
        let escrow_id = env.ledger().sequence();
        
        let escrow = EscrowEntry {
            escrow_id,
            depositor: depositor.clone(),
            beneficiary: beneficiary.clone(),
            amount,
            token: token.clone(),
            purpose: purpose.clone(),
            status: String::from_str(&env, "pending"),
            created_at: current_time,
            lock_until: current_time + lock_duration,
            conditions_met: false,
            release_signature: None,
            lease_id,
        };
        
        // Store escrow entry
        let mut escrows: Map<u64, EscrowEntry> = env.storage().instance()
            .get(&ESCROW_ENTRIES)
            .unwrap_or(Map::new(&env));
        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&ESCROW_ENTRIES, &escrows);
        
        // Transfer tokens to escrow
        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(&depositor, &env.current_contract_address(), &amount);
        
        // Emit event
        env.events().publish(
            (Symbol::short("ESCROW_DEPOSIT"), escrow_id),
            EscrowDepositInitialized {
                escrow_id,
                depositor,
                beneficiary,
                amount,
                token,
                timestamp: current_time,
            }
        );
        
        Ok(escrow_id)
    }

    /// Execute Oracle slashing
    pub fn execute_oracle_slash(
        env: Env,
        oracle: Address,
        escrow_id: u64,
        slash_amount: i128,
        slash_recipient: Address,
    ) -> Result<(), ContractError> {
        // Check freeze status
        Self::check_freeze_status(&env)?;
        
        oracle.require_auth();
        
        let mut escrows: Map<u64, EscrowEntry> = env.storage().instance()
            .get(&ESCROW_ENTRIES)
            .unwrap_or(Map::new(&env));
        let mut escrow: EscrowEntry = escrows.get(escrow_id)
            .ok_or(ContractError::EscrowNotFound)?;
        
        // Validate escrow status
        if escrow.status != String::from_str(&env, "locked") {
            return Err(ContractError::InvalidStatus);
        }
        
        // Validate slash amount
        if slash_amount > escrow.amount {
            return Err(ContractError::InsufficientFunds);
        }
        
        // Update escrow
        escrow.amount -= slash_amount;
        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&ESCROW_ENTRIES, &escrows);
        
        // Transfer slashed amount to recipient
        let token_client = TokenClient::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &slash_recipient, &slash_amount);
        
        // Emit event
        env.events().publish(
            (Symbol::short("ORACLE_SLASH"), escrow_id),
            EscrowOracleSlashExecuted {
                escrow_id,
                slashed_amount: slash_amount,
                slashed_by: oracle,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Execute mutual release
    pub fn execute_mutual_release(
        env: Env,
        releaser: Address,
        escrow_id: u64,
        release_amount: i128,
        recipient: Address,
    ) -> Result<(), ContractError> {
        // Check freeze status
        Self::check_freeze_status(&env)?;
        
        releaser.require_auth();
        
        let mut escrows: Map<u64, EscrowEntry> = env.storage().instance()
            .get(&ESCROW_ENTRIES)
            .unwrap_or(Map::new(&env));
        let mut escrow: EscrowEntry = escrows.get(escrow_id)
            .ok_or(ContractError::EscrowNotFound)?;
        
        // Validate releaser is authorized (depositor or beneficiary)
        if releaser != escrow.depositor && releaser != escrow.beneficiary {
            return Err(ContractError::Unauthorized);
        }
        
        // Validate escrow status
        if escrow.status != String::from_str(&env, "locked") {
            return Err(ContractError::InvalidStatus);
        }
        
        // Validate release amount
        if release_amount > escrow.amount {
            return Err(ContractError::InsufficientFunds);
        }
        
        // Update escrow
        escrow.amount -= release_amount;
        if escrow.amount == 0 {
            escrow.status = String::from_str(&env, "released");
        }
        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&ESCROW_ENTRIES, &escrows);
        
        // Transfer release amount
        let token_client = TokenClient::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &recipient, &release_amount);
        
        // Emit event
        env.events().publish(
            (Symbol::short("MUTUAL_RELEASE"), escrow_id),
            EscrowMutualRelease {
                escrow_id,
                released_to: recipient,
                amount: release_amount,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Deduct arrears
    pub fn deduct_arrears(
        env: Env,
        arrears_collector: Address,
        escrow_id: u64,
        arrears_amount: i128,
        recipient: Address,
    ) -> Result<(), ContractError> {
        // Check freeze status
        Self::check_freeze_status(&env)?;
        
        arrears_collector.require_auth();
        
        let mut escrows: Map<u64, EscrowEntry> = env.storage().instance()
            .get(&ESCROW_ENTRIES)
            .unwrap_or(Map::new(&env));
        let mut escrow: EscrowEntry = escrows.get(escrow_id)
            .ok_or(ContractError::EscrowNotFound)?;
        
        // Validate escrow status
        if escrow.status != String::from_str(&env, "locked") {
            return Err(ContractError::InvalidStatus);
        }
        
        // Validate arrears amount
        if arrears_amount > escrow.amount {
            return Err(ContractError::InsufficientFunds);
        }
        
        // Update escrow
        escrow.amount -= arrears_amount;
        if escrow.amount == 0 {
            escrow.status = String::from_str(&env, "released");
        }
        escrows.set(escrow_id, escrow);
        env.storage().instance().set(&ESCROW_ENTRIES, &escrows);
        
        // Transfer arrears amount
        let token_client = TokenClient::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &recipient, &arrears_amount);
        
        // Emit event
        env.events().publish(
            (Symbol::short("ARREARS_DEDUCTED"), escrow_id),
            EscrowArrearsDeducted {
                escrow_id,
                deducted_amount: arrears_amount,
                deducted_to: recipient,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }

    /// Toggle global escrow freeze (DAO Security Council only)
    pub fn toggle_escrow_freeze(
        env: Env,
        dao_member: Address,
        freeze: bool,
        reason: String,
    ) -> Result<(), ContractError> {
        // Verify DAO Security Council authorization
        let dao_security_council: Address = env.storage().instance().get(&DAO_SECURITY_COUNCIL).unwrap();
        if dao_member != dao_security_council {
            return Err(ContractError::Unauthorized);
        }
        dao_member.require_auth();
        
        let current_freeze_status: bool = env.storage().instance().get(&IS_ESCROW_FROZEN).unwrap_or(false);
        
        if freeze && !current_freeze_status {
            // Activate freeze
            env.storage().instance().set(&IS_ESCROW_FROZEN, &true);
            env.storage().instance().set(&FREEZE_TIMESTAMP, &env.ledger().timestamp());
            
            // Emit critical emergency event
            env.events().publish(
                (Symbol::short("EMERGENCY_FREEZE"), env.ledger().timestamp()),
                EmergencyEscrowFreezeActivated {
                    frozen_by: dao_member,
                    timestamp: env.ledger().timestamp(),
                    reason,
                }
            );
        } else if !freeze && current_freeze_status {
            // Lift freeze
            let freeze_timestamp: u64 = env.storage().instance().get(&FREEZE_TIMESTAMP).unwrap_or(0);
            let frozen_duration = env.ledger().timestamp() - freeze_timestamp;
            
            env.storage().instance().set(&IS_ESCROW_FROZEN, &false);
            env.storage().instance().remove(&FREEZE_TIMESTAMP);
            
            // Emit freeze lifted event
            env.events().publish(
                (Symbol::short("FREEZE_LIFTED"), env.ledger().timestamp()),
                EmergencyEscrowFreezeLifted {
                    lifted_by: dao_member,
                    frozen_duration,
                    timestamp: env.ledger().timestamp(),
                }
            );
        }
        
        Ok(())
    }

    /// Check if escrow is frozen
    pub fn is_escrow_frozen(env: Env) -> bool {
        env.storage().instance().get(&IS_ESCROW_FROZEN).unwrap_or(false)
    }

    /// Get escrow entry
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<EscrowEntry, ContractError> {
        let escrows: Map<u64, EscrowEntry> = env.storage().instance()
            .get(&ESCROW_ENTRIES)
            .unwrap_or(Map::new(&env));
        
        escrows.get(escrow_id)
            .ok_or(ContractError::EscrowNotFound)
    }

    /// Get freeze timestamp
    pub fn get_freeze_timestamp(env: Env) -> Option<u64> {
        env.storage().instance().get(&FREEZE_TIMESTAMP)
    }

    /// Internal function to check freeze status
    fn check_freeze_status(env: &Env) -> Result<(), ContractError> {
        let is_frozen: bool = env.storage().instance().get(&IS_ESCROW_FROZEN).unwrap_or(false);
        if is_frozen {
            return Err(ContractError::EscrowFrozen);
        }
        Ok(())
    }

    /// Set DAO Security Council (admin only)
    pub fn set_dao_security_council(
        env: Env,
        admin: Address,
        new_dao_council: Address,
    ) -> Result<(), ContractError> {
        let contract_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        if contract_admin != admin {
            return Err(ContractError::Unauthorized);
        }
        admin.require_auth();
        
        env.storage().instance().set(&DAO_SECURITY_COUNCIL, &new_dao_council);
        Ok(())
    }

    /// Get contract version
    pub fn get_version(env: Env) -> String {
        env.storage().instance()
            .get(&CONTRACT_VERSION)
            .unwrap_or_else(|| "1.0.0".into_val(&env))
    }
}
