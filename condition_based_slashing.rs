use soroban_sdk::{contracttype, contracterror, Address, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SlashingError {
    UnauthorizedOracle = 50,
    DamageExceedsDeposit = 51,
    AssessmentExpired = 52,
    InvalidAssessmentState = 53,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DamageAssessment {
    pub lease_id: u64,
    pub assessed_damage_amount: i128,
    pub oracle_address: Address,
    pub condition_report_hash: BytesN<32>, // Data hash representing the IPFS report
    pub timestamp: u64,
}

pub trait ConditionBasedSlashing {
    /// Executes a precise deposit slash based on an Oracle-verified condition report
    fn execute_conditional_slash(
        env: Env,
        lease_id: u64,
        assessment: DamageAssessment,
        available_deposit: i128,
    ) -> Result<i128, SlashingError>;
}

pub struct ConditionSlashingModule;

impl ConditionBasedSlashing for ConditionSlashingModule {
    fn execute_conditional_slash(
        env: Env,
        lease_id: u64,
        assessment: DamageAssessment,
        available_deposit: i128,
    ) -> Result<i128, SlashingError> {
        // 1. Validate the oracle authorization (Assuming a verified oracle signature is validated prior)
        assessment.oracle_address.require_auth();

        // 2. Ensure assessment is recent (e.g., within 7 days)
        let current_time = env.ledger().timestamp();
        if current_time > assessment.timestamp + 604800 {
            return Err(SlashingError::AssessmentExpired);
        }

        // 3. Calculate slash amount with protective bounds (never exceed available deposit)
        let slash_amount = if assessment.assessed_damage_amount > available_deposit {
            available_deposit
        } else {
            assessment.assessed_damage_amount
        };
        
        // Note: The caller (main contract) will route the `slash_amount` to the lessor 
        // and return the `available_deposit - slash_amount` to the lessee.

        Ok(slash_amount)
    }
}