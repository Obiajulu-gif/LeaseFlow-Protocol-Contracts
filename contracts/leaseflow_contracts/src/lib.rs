#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::enum_variant_names)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address,
    BytesN, Env, String, Symbol, Vec,
};

mod velocity_guard;
use velocity_guard::VelocityGuard;

mod continuous_billing_module;
use continuous_billing_module::ContinuousBillingModule;

#[cfg(test)]
mod velocity_guard_tests;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    PerSecond,
    PerHour,
    PerDay,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositStatus {
    Held,
    Settled,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
    Terminated,
    DaoArbitration,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaintenanceStatus {
    None,
    Reported,
    Fixed,
    Verified,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageSeverity {
    NormalWearAndTear = 0,
    Minor = 1,
    Moderate = 2,
    Major = 3,
    Severe = 4,
    Catastrophic = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleStatus {
    Active,
    Demoted,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OraclePayload {
    pub lease_id: u64,
    pub oracle_pubkey: BytesN<32>,
    pub damage_severity: DamageSeverity,
    pub nonce: u64,
    pub timestamp: u64,
    pub signature: BytesN<64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleTier {
    Primary,
    Backup,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfig {
    pub pubkey: BytesN<32>,
    pub tier: OracleTier,
    pub status: OracleStatus,
    pub last_successful_timestamp: u64,
    pub demotion_timestamp: Option<u64>,
    pub failure_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackHierarchy {
    pub primary_oracle: BytesN<32>,
    pub backup_oracle: BytesN<32>,
    pub dao_arbitration_enabled: bool,
    pub hierarchy_active: bool,
    pub last_demotion_time: Option<u64>,
}

#[contracttype]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositReleasePartial {
    pub tenant_amount: i128,
    pub landlord_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_per_sec: i128,
    pub late_fee_per_sec: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub debt: i128,
    pub flat_fee_applied: bool,
    pub seconds_late_charged: u64,
    pub rent_paid: i128,
    pub expiry_time: u64,
    pub buyout_price: Option<i128>,
    pub cumulative_payments: i128,
    pub payment_token: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseInstance {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub security_deposit: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub rent_paid: i128,
    pub expiry_time: u64,
    pub buyout_price: Option<i128>,
    pub cumulative_payments: i128,
    pub debt: i128,
    pub rent_paid_through: u64,
    pub deposit_status: DepositStatus,
    pub rent_per_sec: i128,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub late_fee_per_sec: i128,
    pub flat_fee_applied: bool,
    pub seconds_late_charged: u64,
    pub withdrawal_address: Option<Address>,
    pub rent_withdrawn: i128,
    pub arbitrators: soroban_sdk::Vec<Address>,
    pub maintenance_status: MaintenanceStatus,
    pub withheld_rent: i128,
    pub repair_proof_hash: Option<BytesN<32>>,
    pub inspector: Option<Address>,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub paused_at: Option<u64>,
    pub pause_initiator: Option<Address>,
    pub total_paused_duration: u64,
    pub rent_pull_authorized_amount: Option<i128>,
    pub last_rent_pull_timestamp: Option<u64>,
    pub billing_cycle_duration: u64,
    pub continuous_billing_enabled: bool,
    pub rent_treasury_address: Option<Address>,
    pub yield_delegation_enabled: bool,
    pub yield_accumulated: i128,
    pub equity_balance: i128,
    pub equity_percentage_bps: u32,
    pub had_late_payment: bool,
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    pub payment_token: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub lease_id: Symbol,
    pub month: u32,
    pub amount: i128,
    pub date: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRights {
    pub renter: Address,
    pub nft_contract: Address,
    pub token_id: u128,
    pub lease_id: Symbol,
    pub valid_until: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseAmendment {
    pub new_rent_per_sec: Option<i128>,
    pub new_end_date: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateLeaseParams {
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub security_deposit: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub payment_token: Address,
    pub arbitrators: Vec<Address>,
    pub rent_per_sec: i128,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub late_fee_per_sec: i128,
    pub equity_percentage_bps: u32,
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    pub yield_delegation_enabled: bool,
    pub deposit_asset: Option<Address>,
    pub dex_contract: Option<Address>,
    pub max_slippage_bps: u32,
    pub swap_path: Vec<Address>,
}

#[contracttype]
pub enum DataKey {
    Lease(Symbol),
    LeaseInstance(u64),
    Receipt(Symbol, u32),
    Admin,
    UsageRights(Address, u128),
    HistoricalLease(u64),
    KycProvider,
    AllowedAsset(Address),
    AuthorizedPayer(u64, Address),
    RoommateBalance(u64, Address),
    PlatformFeeAmount,
    PlatformFeeToken,
    PlatformFeeRecipient,
    TermsHash,
    WhitelistedOracle(BytesN<32>),
    OracleNonce(BytesN<32>, u64),
    TenantFlag(u64),
    YieldDeployment(u64),
    WhitelistedYieldProtocol(Address),
    LiquidityBuffer,
    YieldAccumulated(u64),
    OracleConfig(BytesN<32>),
    FallbackHierarchy,
    OracleFailureTimestamp(BytesN<32>),
    // Issue #117: Multi-Sig Veto
    SecurityCouncil,
    PendingSlash(u64),
    VetoVote(u64, Address),
    // Issue #118: Dynamic Protocol Fees
    ProtocolFeeConfig,
    PendingFeeUpdate,
    // Issue #119: Quadratic Voting
    GovernanceRound(u64),
    TreasuryVote(u64, Address),
    VotingPowerSnapshot(u64, Address),
    // Issue #124: Active Leases Index
    ActiveLeasesIndex,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalLease {
    pub lease: LeaseInstance,
    pub terminated_by: Address,
    pub terminated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldDeployment {
    pub lease_id: u64,
    pub principal_amount: i128,
    pub yield_protocol: Address,
    pub deployment_timestamp: u64,
    pub lp_tokens: i128,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldDistribution {
    pub lessee_bps: u32,
    pub lessor_bps: u32,
    pub dao_bps: u32,
}

// ============================================================================
// Issue #117: Multi-Sig Veto on Massive Deposit Slashing
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityCouncilMember {
    pub address: Address,
    pub voting_power: u32, // Weighted voting power
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityCouncil {
    pub members: soroban_sdk::Vec<SecurityCouncilMember>,
    pub veto_threshold_bps: u32, // Basis points required for veto (e.g., 6000 = 60%)
    pub total_voting_power: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSlashVeto {
    pub lease_id: u64,
    pub slash_amount: i128,
    pub tenant_refund: i128,
    pub landlord_payout: i128,
    pub oracle_payload: OraclePayload,
    pub proposed_at: u64,
    pub timelock_end: u64,
    pub veto_votes_for: u32,
    pub veto_votes_against: u32,
    pub executed: bool,
    pub vetoed: bool,
}

// ============================================================================
// Issue #118: DAO-Governed Dynamic Protocol Fee Updates
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFeeConfig {
    pub current_fee_bps: u32, // Current fee in basis points
    pub max_fee_bps: u32,     // Maximum allowed fee (hard cap)
    pub min_fee_bps: u32,     // Minimum allowed fee
    pub max_increase_bps: u32, // Maximum increase per update (e.g., 500 = 5%)
    pub update_timelock: u64,  // Timelock period before changes take effect
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFeeUpdate {
    pub proposed_fee_bps: u32,
    pub proposed_by: Address,
    pub proposed_at: u64,
    pub execution_time: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub executed: bool,
}

// ============================================================================
// Issue #119: Quadratic Voting for Treasury Yield Allocation
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceRound {
    pub round_id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub total_treasury_yield: i128,
    pub allocation_options: soroban_sdk::Vec<AllocationOption>,
    pub active: bool,
    pub snapshot_timestamp: u64, // Snapshot to prevent flash loan attacks
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationOption {
    pub option_id: u32,
    pub description: String,
    pub total_quadratic_votes: i128, // Sum of sqrt(voting_power)
    pub recipient_address: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasuryVote {
    pub round_id: u64,
    pub voter: Address,
    pub option_id: u32,
    pub tokens_committed: i128, // Tokens committed for voting
    pub voting_power: i128,     // Quadratic voting power: sqrt(tokens)
    pub voted_at: u64,
}

// ============================================================================
// Issue #124: Highly Optimized get_active_leases Read-Only Query
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveLeaseSummary {
    pub lease_id: u64,
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub rent_per_sec: i128,
    pub deposit_amount: i128,
    pub security_deposit: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
    pub payment_token: Address,
    pub rent_paid: i128,
    pub cumulative_payments: i128,
    pub debt: i128,
    pub active: bool,
    pub yield_delegation_enabled: bool,
    pub equity_percentage_bps: u32,
}

#[contractevent]
pub struct RoommateAdded {
    pub lease_id: u64,
    pub roommate: Address,
}

#[contractevent]
pub struct RentPaidPartial {
    pub lease_id: u64,
    pub roommate: Address,
    pub amount: i128,
}

#[contractevent]
pub struct LeaseStarted {
    pub id: u64,
    pub renter: Address,
    pub rate: i128,
}

#[contractevent]
pub struct LeaseEnded {
    pub id: u64,
    pub duration: u64,
    pub total_paid: i128,
}

#[contractevent]
pub struct AssetReclaimed {
    pub id: u64,
    pub reason: String,
}

#[contractevent]
pub struct LeaseTerminated {
    pub lease_id: u64,
}

#[contractevent]
pub struct TerminateBountyPaid {
    pub lease_id: u64,
    pub caller: Address,
    pub amount: i128,
}

#[contractevent]
pub struct MaintenanceIssueReported {
    pub lease_id: u64,
    pub tenant: Address,
}

#[contractevent]
pub struct RepairProofSubmitted {
    pub lease_id: u64,
    pub landlord: Address,
    pub proof_hash: BytesN<32>,
}

#[contractevent]
pub struct MaintenanceVerified {
    pub lease_id: u64,
    pub inspector: Address,
    pub withheld_released: i128,
}

#[contractevent]
pub struct ContractUpgraded {
    pub old_wasm_hash: BytesN<32>,
    pub new_wasm_hash: BytesN<32>,
}

#[contractevent]
pub struct TermsHashUpdated {
    pub new_terms_hash: BytesN<32>,
}

#[contractevent]
pub struct DepositDisputed {
    pub lease_id: u64,
    pub caller: Address,
}

#[contractevent]
pub struct DisputeResolved {
    pub lease_id: u64,
    pub resolution: DepositReleasePartial,
}

#[contractevent]
pub struct EvictionEligible {
    pub lease_id: u64,
    pub tenant: Address,
    pub debt: i128,
}

#[contractevent]
pub struct CrossAssetDepositLocked {
    pub lease_id: u64,
    pub original_asset: Address,
    pub collateral_asset: Address,
    pub swap_path: Vec<Address>,
    pub original_amount: i128,
    pub final_locked_amount: i128,
}

#[contractevent]
pub struct LeaseSigned {
    pub lease_id: u64,
    pub property_hash: String,
}

#[contractevent]
pub struct PaymentLate {
    pub lease_id: u64,
    pub days_late: u64,
    pub current_fine: i128,
}

#[contractevent]
pub struct MutualLeaseFinalized {
    pub lease_id: u64,
    pub return_amount: i128,
    pub slash_amount: i128,
    pub tenant_refund: i128,
    pub landlord_payout: i128,
}

#[contractevent]
pub struct DepositSlashed {
    pub lease_id: u64,
    pub oracle_pubkey: BytesN<32>,
    pub damage_code: u32,
    pub deducted_amount: i128,
    pub tenant_refund: i128,
    pub landlord_payout: i128,
}

#[contractevent]
pub struct TenantFlagged {
    pub lease_id: u64,
    pub tenant: Address,
    pub reason: String,
}

#[contractevent]
pub struct EscrowYieldHarvested {
    pub lease_id: u64,
    pub total_yield: i128,
    pub lessee_share: i128,
    pub lessor_share: i128,
    pub dao_share: i128,
    pub yield_protocol: Address,
    pub harvest_timestamp: u64,
}

#[contractevent]
pub struct OracleDemoted {
    pub oracle_pubkey: BytesN<32>,
    pub reason: String,
    pub demotion_timestamp: u64,
    pub failure_count: u32,
}

#[contractevent]
pub struct FallbackHierarchyActivated {
    pub primary_oracle: BytesN<32>,
    pub backup_oracle: BytesN<32>,
    pub activation_timestamp: u64,
    pub reason: String,
}

#[contractevent]
pub struct DaoArbitrationTriggered {
    pub lease_id: u64,
    pub trigger_timestamp: u64,
    pub reason: String,
}

#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    LeaseNotFound = 1,
    LeaseNotExpired = 2,
    RentOutstanding = 3,
    DepositNotSettled = 4,
    Unauthorised = 5,
    InvalidDeduction = 6,
    NftTransferFailed = 7,
    UsageRightsNotFound = 8,
    UsageRightsExpired = 9,
    KycRequired = 10,
    InvalidAsset = 11,
    NftNotReturned = 12,
    WithdrawalAddressNotSet = 13,
    NotAnArbitrator = 14,
    LeaseAlreadyExists = 15,
    UpgradeNotAllowed = 16,
    PathPaymentFailed = 17,
    SlippageExceeded = 18,
    InvalidReleaseMath = 19,
    OracleNotWhitelisted = 20,
    InvalidSignature = 21,
    InvalidNonce = 22,
    LeaseNotTerminated = 23,
    DepositAlreadySettled = 24,
    YieldUnderflow = 25,
    InsufficientLiquidityBuffer = 26,
    YieldProtocolNotWhitelisted = 27,
    InvalidYieldDistribution = 28,
    OracleStale = 29,
    OracleUnavailable = 30,
    FallbackHierarchyNotConfigured = 31,
    DaoArbitrationNotEnabled = 32,
    OracleBypassAttempt = 33,
    // Issue #117: Multi-Sig Veto errors
    PendingVeto = 34,
    TimelockNotExpired = 35,
    AlreadyVoted = 36,
    InvalidState = 37,
    // Issue #118: Dynamic Fee errors
    ProposalRejected = 38,
    InvalidParameters = 39,
    // Issue #119: Quadratic Voting errors
    GovernanceRoundEnded = 40,
    GovernanceRoundActive = 41,
}

macro_rules! require {
    ($condition:expr, $error_msg:expr) => {
        if !$condition {
            panic!($error_msg);
        }
    };
}

const DAY_IN_LEDGERS: u32 = 17280;
const MONTH_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 30;
const YEAR_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 365;
const STALENESS_THRESHOLD: u64 = 48 * 60 * 60; // 48 hours in seconds
const BACKUP_FAILURE_THRESHOLD: u64 = 7 * 24 * 60 * 60; // 7 days in seconds
const MAX_ORACLE_FAILURES: u32 = 3;

// Issue #117: Multi-Sig Veto Constants
const MASSIVE_SLASH_THRESHOLD: i128 = 10000_0000000; // 10,000 tokens (in smallest units)
const VETO_TIMELOCK_PERIOD: u64 = 24 * 60 * 60; // 24 hours timelock for massive slashes
const DEFAULT_VETO_THRESHOLD_BPS: u32 = 6000; // 60% of council voting power

// Issue #118: Dynamic Protocol Fee Constants
const DEFAULT_MAX_FEE_BPS: u32 = 3000; // 30% max fee cap
const DEFAULT_MIN_FEE_BPS: u32 = 0;    // 0% minimum fee
const DEFAULT_MAX_INCREASE_BPS: u32 = 500; // 5% max increase per update
const DEFAULT_FEE_TIMELOCK: u64 = 7 * 24 * 60 * 60; // 7 days timelock
const DEFAULT_PROTOCOL_FEE_BPS: u32 = 100; // 1% default fee

// Issue #119: Quadratic Voting Constants
const GOVERNANCE_ROUND_DURATION: u64 = 7 * 24 * 60 * 60; // 7 days voting period
const FLASH_LOAN_PROTECTION_BUFFER: u64 = 24 * 60 * 60; // 24 hours snapshot before round

pub fn to_per_second(rate: i128, rate_type: RateType) -> i128 {
    match rate_type {
        RateType::PerSecond => rate,
        RateType::PerHour => rate / 3_600,
        RateType::PerDay => rate / 86_400,
    }
}

pub fn save_lease(env: &Env, lease_id: &Symbol, lease: &Lease) {
    let key = DataKey::Lease(lease_id.clone());
    env.storage().persistent().set(&key, lease);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_lease_by_id(env: &Env, lease_id: &Symbol) -> Option<Lease> {
    env.storage()
        .persistent()
        .get(&DataKey::Lease(lease_id.clone()))
}

pub fn save_lease_instance(env: &Env, lease_id: u64, lease: &LeaseInstance) {
    let key = DataKey::LeaseInstance(lease_id);
    env.storage().persistent().set(&key, lease);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_lease_instance_by_id(env: &Env, lease_id: u64) -> Option<LeaseInstance> {
    env.storage()
        .persistent()
        .get(&DataKey::LeaseInstance(lease_id))
}

pub fn delete_lease_instance(env: &Env, lease_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::LeaseInstance(lease_id));
}

pub fn save_usage_rights(
    env: &Env,
    nft_contract: Address,
    token_id: u128,
    usage_rights: &UsageRights,
) {
    env.storage()
        .instance()
        .set(&DataKey::UsageRights(nft_contract, token_id), usage_rights);
}

pub fn delete_usage_rights(env: &Env, nft_contract: Address, token_id: u128) {
    env.storage()
        .instance()
        .remove(&DataKey::UsageRights(nft_contract, token_id));
}

pub fn load_usage_rights(env: &Env, nft_contract: Address, token_id: u128) -> Option<UsageRights> {
    env.storage()
        .instance()
        .get(&DataKey::UsageRights(nft_contract, token_id))
}

pub fn archive_lease(env: &Env, lease_id: u64, lease: LeaseInstance, caller: Address) {
    let historical = HistoricalLease {
        lease,
        terminated_by: caller,
        terminated_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::HistoricalLease(lease_id), &historical);
    delete_lease_instance(env, lease_id);
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

mod token_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "TokenClient")]
    pub trait TokenInterface {
        fn transfer(env: Env, from: Address, to: Address, amount: i128);
    }
}

mod kyc_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "KycClient")]
    pub trait KycInterface {
        fn is_verified(env: Env, address: Address) -> bool;
    }
}

mod dex_contract {
    use soroban_sdk::{contractclient, Address, Env, Vec};
    #[contractclient(name = "DexClient")]
    pub trait DexInterface {
        fn path_payment(
            env: Env,
            from: Address,
            to: Address,
            amount_in: i128,
            max_slippage_bps: u32,
            path: Vec<Address>,
        ) -> i128;
    }
}

mod yield_protocol {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "YieldClient")]
    pub trait YieldInterface {
        fn deposit(env: Env, from: Address, amount: i128) -> i128;
        fn withdraw(env: Env, from: Address, lp_tokens: i128) -> i128;
        fn get_balance(env: Env, user: Address) -> i128;
        fn claim_rewards(env: Env, user: Address) -> i128;
    }
}

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    fn require_stablecoin(env: &Env, token: &Address) -> Result<(), LeaseError> {
        if !Self::is_asset_allowed(env, token) {
            return Err(LeaseError::InvalidAsset);
        }
        Ok(())
    }

    fn execute_deposit_swap(
        env: &Env,
        lease_id: u64,
        tenant: &Address,
        original_asset: &Address,
        collateral_asset: &Address,
        original_amount: i128,
        max_slippage_bps: u32,
        swap_path: &Vec<Address>,
        dex_contract: &Option<Address>,
    ) -> Result<i128, LeaseError> {
        if original_asset == collateral_asset {
            return Ok(original_amount);
        }
        if swap_path.is_empty() {
            return Err(LeaseError::PathPaymentFailed);
        }
        let final_locked_amount = if let Some(dex_addr) = dex_contract {
            let dex_client = dex_contract::DexClient::new(env, dex_addr);
            dex_client.path_payment(
                tenant,
                collateral_asset,
                &original_amount,
                &max_slippage_bps,
                swap_path,
            )
        } else {
            let simulated_output = original_amount.saturating_mul(9_900) / 10_000;
            let min_out =
                original_amount.saturating_mul(10_000i128 - max_slippage_bps as i128) / 10_000i128;
            if simulated_output < min_out {
                return Err(LeaseError::SlippageExceeded);
            }
            simulated_output
        };
        CrossAssetDepositLocked {
            lease_id,
            original_asset: original_asset.clone(),
            collateral_asset: collateral_asset.clone(),
            swap_path: swap_path.clone(),
            original_amount,
            final_locked_amount,
        }
        .publish(env);
        let _ = tenant;
        Ok(final_locked_amount)
    }

    fn is_asset_allowed(env: &Env, token: &Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::AllowedAsset(token.clone()))
    }

    /// Adds a token address to the allowlist of accepted payment assets.
    ///
    /// # Parameters
    /// - `admin` – Must match the stored admin address.
    /// - `asset` – Token contract address to whitelist.
    ///
    /// # Errors
    /// - [`LeaseError::Unauthorised`] – Caller is not the stored admin.
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn add_allowed_asset(env: Env, admin: Address, asset: Address) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::AllowedAsset(asset), &true);
        Ok(())
    }

    fn require_kyc(env: &Env, landlord: &Address, tenant: &Address) -> Result<(), LeaseError> {
        if let Some(provider_addr) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::KycProvider)
        {
            let client = kyc_contract::KycClient::new(env, &provider_addr);
            if !client.is_verified(landlord) || !client.is_verified(tenant) {
                return Err(LeaseError::KycRequired);
            }
        }
        Ok(())
    }

    /// Sets the on-chain KYC provider contract address.
    ///
    /// # Parameters
    /// - `admin`    – Must match the stored admin address.
    /// - `provider` – Address of the KYC verification contract.
    ///
    /// # Errors
    /// - [`LeaseError::Unauthorised`] – Caller is not the stored admin.
    ///
    /// # Authorization
    /// Requires `admin.require_auth()`.
    pub fn set_kyc_provider(env: Env, admin: Address, provider: Address) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::KycProvider, &provider);
        Ok(())
    }

    pub fn initialize_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        _rent_amount: i128,
        deposit_amount: i128,
        duration: u64,
        property_uri: String,
        payment_token: Address,
    ) -> Result<bool, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        let start_date = env.ledger().timestamp();
        let end_date = start_date.saturating_add(duration);
        let lease = Lease {
            landlord,
            tenant,
            rent_per_sec: 0,
            late_fee_per_sec: 0,
            deposit_amount,
            start_date,
            end_date,
            property_uri,
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            active: true,
            grace_period_end: end_date,
            late_fee_flat: 0,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time: end_date,
            buyout_price: None,
            cumulative_payments: 0,
            payment_token,
        };
        env.storage().instance().set(&lease_id, &lease);
        Ok(true)
    }

    pub fn create_lease(
        env: Env,
        landlord: Address,
        tenant: Address,
        _amount: i128,
        payment_token: Address,
    ) -> Result<Symbol, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        let lease_id = symbol_short!("lease");
        let lease = Lease {
            landlord,
            tenant,
            rent_per_sec: 0,
            late_fee_per_sec: 0,
            deposit_amount: 0,
            start_date: env.ledger().timestamp(),
            end_date: 0,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            active: true,
            grace_period_end: 0,
            late_fee_flat: 0,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time: 0,
            buyout_price: None,
            cumulative_payments: 0,
            payment_token,
        };
        env.storage().instance().set(&lease_id, &lease);
        Ok(lease_id)
    }

    pub fn create_lease_with_continuous_billing(
        env: Env,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        rent_per_second: i128,
        deposit_amount: i128,
        duration: u64,
        property_uri: String,
        payment_token: Address,
        billing_frequency: u64,
        rent_treasury_address: Address,
    ) -> Result<u64, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        
        let current_time = env.ledger().timestamp();
        let lease_id = env.ledger().sequence();
        let end_time = current_time.saturating_add(duration);
        
        // Create lease instance with continuous billing support
        let lease_instance = LeaseInstance {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount,
            deposit_amount,
            security_deposit: deposit_amount,
            start_date: current_time,
            end_date: end_time,
            property_uri: property_uri.clone(),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: end_time,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: current_time,
            deposit_status: DepositStatus::Held,
            rent_per_sec: rent_per_second,
            grace_period_end: end_time,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: Vec::new(&env),
            maintenance_status: MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
            paused: false,
            pause_reason: None,
            paused_at: None,
            pause_initiator: None,
            total_paused_duration: 0,
            rent_pull_authorized_amount: Some(rent_amount * 12), // 12 months authorization
            last_rent_pull_timestamp: Some(current_time),
            billing_cycle_duration: billing_frequency,
            continuous_billing_enabled: true,
            rent_treasury_address: Some(rent_treasury_address.clone()),
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            payment_token: payment_token.clone(),
        };
        
        // Save lease instance
        save_lease_instance(&env, &lease_id, &lease_instance);
        
        // Register lease with continuous billing module
        ContinuousBillingModule::register_lease_billing(
            env.clone(),
            lease_id,
            landlord.clone(),
            tenant.clone(),
            Address::from_string(&env, "property_asset"), // Placeholder for asset address
            rent_amount,
            rent_per_second,
            payment_token.clone(),
            current_time,
            end_time,
            billing_frequency,
        ).map_err(|_| LeaseError::Unauthorised)?; // Convert billing module error
        
        // Emit lease creation event
        LeaseStarted {
            id: lease_id,
            renter: tenant,
            rate: rent_per_second,
        }
        .publish(&env);
        
        Ok(lease_id)
    }

    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");
        require!(lease.tenant == tenant, "Unauthorized");
        lease.status = LeaseStatus::Active;
        env.storage().instance().set(&lease_id, &lease);
        LeaseStarted {
            id: env.ledger().timestamp(),
            renter: tenant,
            rate: lease.rent_per_sec,
        }
        .publish(&env);
        symbol_short!("active")
    }

    pub fn pay_rent(
        env: Env,
        lease_id: Symbol,
        payment_amount: i128,
    ) -> Result<Symbol, LeaseError> {
        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");
        require!(lease.active, "Lease is not active");
        Self::require_kyc(&env, &lease.landlord, &lease.tenant)?;
        Self::require_stablecoin(&env, &lease.payment_token)?;
        lease.cumulative_payments += payment_amount;

        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id)
                {
                    let nft_client = nft_contract::NftClient::new(&env, nft_contract);
                    nft_client.transfer_from(
                        &env.current_contract_address(),
                        &env.current_contract_address(),
                        &lease.tenant,
                        &token_id,
                    );
                }
            }
        }
        env.storage().instance().set(&lease_id, &lease);
        Ok(symbol_short!("paid"))
    }

    pub fn pay_rent_receipt(env: Env, lease_id: Symbol, month: u32, amount: i128) -> bool {
        let receipt = Receipt {
            lease_id,
            month,
            amount,
            date: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::Receipt(receipt.lease_id.clone(), month), &receipt);
        true
    }

    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found")
    }

    pub fn get_lease_default(env: Env) -> Lease {
        env.storage()
            .instance()
            .get(&symbol_short!("lease"))
            .expect("Lease not found")
    }

    pub fn set_buyout_price(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        buyout_price: i128,
    ) -> Symbol {
        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");
        require!(lease.landlord == landlord, "Unauthorized");
        lease.buyout_price = Some(buyout_price);
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("buyout")
    }

    pub fn get_receipt(env: Env, lease_id: Symbol, month: u32) -> Receipt {
        env.storage()
            .instance()
            .get(&DataKey::Receipt(lease_id, month))
            .expect("Receipt not found")
    }

    pub fn end_lease(env: Env, lease_id: Symbol, caller: Address) -> Symbol {
        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");
        require!(
            lease.landlord == caller || lease.tenant == caller,
            "Unauthorized"
        );
        caller.require_auth();
        if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id) {
            delete_usage_rights(&env, nft_contract.clone(), *token_id);
            let nft_client = nft_contract::NftClient::new(&env, nft_contract);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                token_id,
            );
        }
        lease.status = LeaseStatus::Terminated;
        lease.active = false;
        env.storage().instance().set(&lease_id, &lease);
        LeaseEnded {
            id: env.ledger().timestamp(),
            duration: env.ledger().timestamp() - lease.start_date,
            total_paid: lease.cumulative_payments,
        }
        .publish(&env);
        symbol_short!("ended")
    }

    pub fn extend_ttl(env: Env, _lease_id: Symbol) {
        env.storage()
            .instance()
            .extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
    }

    pub fn check_usage_rights(
        env: Env,
        nft_contract: Address,
        token_id: u128,
        user: Address,
    ) -> Option<UsageRights> {
        if let Some(rights) = load_usage_rights(&env, nft_contract, token_id) {
            if rights.renter == user && env.ledger().timestamp() <= rights.valid_until {
                return Some(rights);
            }
        }
        None
    }

    /// Creates a new `LeaseInstance` in persistent storage.
    ///
    /// Validates KYC, whitelisted payment token, and uniqueness of `lease_id`.
    /// Optionally swaps the tenant's deposit asset via a DEX path payment before
    /// locking it as collateral. Initialises the velocity guard for the landlord
    /// and adds the lease to the active-leases index.
    ///
    /// # Parameters
    /// - `lease_id` – Unique numeric identifier for this lease.
    /// - `landlord` – Landlord address; must be KYC-verified.
    /// - `params`   – Full set of lease creation parameters (see [`CreateLeaseParams`]).
    ///
    /// # Errors
    /// - [`LeaseError::LeaseAlreadyExists`] – A lease with `lease_id` already exists.
    /// - [`LeaseError::KycRequired`]        – Landlord or tenant is not KYC-verified.
    /// - [`LeaseError::InvalidAsset`]       – Payment token is not on the allowlist.
    /// - [`LeaseError::PathPaymentFailed`]  – DEX swap path is empty.
    /// - [`LeaseError::SlippageExceeded`]   – DEX swap output is below `max_slippage_bps`.
    ///
    /// # Authorization
    /// Requires `landlord.require_auth()` and `params.tenant.require_auth()`.
    pub fn create_lease_instance(
        env: Env,
        lease_id: u64,
        landlord: Address,
        params: CreateLeaseParams,
    ) -> Result<(), LeaseError> {
        if env
            .storage()
            .persistent()
            .has(&DataKey::LeaseInstance(lease_id))
        {
            return Err(LeaseError::LeaseAlreadyExists);
        }
        landlord.require_auth();
        params.tenant.require_auth();
        let locked_amount = if let Some(deposit_asset) = params.deposit_asset.clone() {
            Self::execute_deposit_swap(
                &env,
                lease_id,
                &params.tenant,
                &deposit_asset,
                &params.payment_token,
                params.security_deposit,
                params.max_slippage_bps,
                &params.swap_path,
                &params.dex_contract,
            )?
        } else {
            params.security_deposit
        };
        let lease = LeaseInstance {
            landlord,
            tenant: params.tenant,
            rent_amount: params.rent_amount,
            deposit_amount: params.deposit_amount,
            security_deposit: locked_amount,
            start_date: params.start_date,
            end_date: params.end_date,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            status: LeaseStatus::Pending,
            property_uri: params.property_uri.clone(),
            nft_contract: None,
            token_id: None,
            active: true,
            debt: 0,
            rent_paid: 0,
            expiry_time: params.end_date,
            buyout_price: None,
            cumulative_payments: 0,
            rent_per_sec: params.rent_per_sec,
            grace_period_end: params.grace_period_end,
            late_fee_flat: params.late_fee_flat,
            late_fee_per_sec: params.late_fee_per_sec,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: params.arbitrators,
            maintenance_status: MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
            paused: false,
            pause_reason: None,
            paused_at: None,
            pause_initiator: None,
            total_paused_duration: 0,
            rent_pull_authorized_amount: None,
            last_rent_pull_timestamp: None,
            billing_cycle_duration: 2_592_000,
            yield_delegation_enabled: params.yield_delegation_enabled,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: params.equity_percentage_bps,
            had_late_payment: false,
            has_pet: params.has_pet,
            pet_deposit_amount: params.pet_deposit_amount,
            pet_rent_amount: params.pet_rent_amount,
            payment_token: params.payment_token.clone(),
        };
        save_lease_instance(&env, lease_id, &lease);

        // Add to active leases index for optimized querying (Issue #124)
        Self::add_to_active_leases_index(&env, lease_id)?;

        // Initialize velocity tracker for landlord and update portfolio size
        VelocityGuard::initialize_lessor(&env, &landlord)?;
        VelocityGuard::update_portfolio_size(&env, &landlord, 1)?;

        LeaseSigned {
            lease_id,
            property_hash: params.property_uri,
        }
        .publish(&env);
        Ok(())
    }

    /// Returns the [`LeaseInstance`] for `lease_id`.
    ///
    /// # Errors
    /// - [`LeaseError::LeaseNotFound`] – No lease exists for `lease_id`.
    pub fn get_lease_instance(env: Env, lease_id: u64) -> Result<LeaseInstance, LeaseError> {
        load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)
    }

    pub fn set_lease_instance_buyout_price(
        env: Env,
        lease_id: u64,
        landlord: Address,
        buyout_price: i128,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();
        lease.buyout_price = Some(buyout_price);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    /// Accepts a rent payment from the primary tenant or an authorised co-payer.
    ///
    /// Transfers `payment_amount` tokens from `payer` to the contract, updates
    /// cumulative accounting, and triggers buyout logic if the total crosses
    /// `buyout_price`.
    ///
    /// # Parameters
    /// - `lease_id`       – ID of the target lease.
    /// - `payer`          – Address making the payment.
    /// - `payment_amount` – Amount in the lease's payment token (smallest unit).
    ///
    /// # Errors
    /// - [`LeaseError::LeaseNotFound`] – No lease exists for `lease_id`.
    /// - [`LeaseError::Unauthorised`]  – `payer` is neither the tenant nor an authorised payer.
    ///
    /// # Authorization
    /// Requires `payer.require_auth()`.
    pub fn pay_lease_instance_rent(
        env: Env,
        lease_id: u64,
        payer: Address,
        payment_amount: i128,
    ) -> Result<(), LeaseError> {
        payer.require_auth();

        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        require!(lease.active, "Lease is not active");

        let is_primary = payer == lease.tenant;
        let is_authorized = env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::AuthorizedPayer(lease_id, payer.clone()))
            .unwrap_or(false);
        if !is_primary && !is_authorized {
            return Err(LeaseError::Unauthorised);
        }

        let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
        token_client.transfer(&payer, &env.current_contract_address(), &payment_amount);

        let balance_key = DataKey::RoommateBalance(lease_id, payer.clone());
        let mut payer_bal: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        payer_bal += payment_amount;
        env.storage().persistent().set(&balance_key, &payer_bal);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);

        lease.cumulative_payments += payment_amount;
        lease.rent_paid += payment_amount;

        RentPaidPartial {
            lease_id,
            roommate: payer.clone(),
            amount: payment_amount,
        }
        .publish(&env);

        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                if let (Some(nft), Some(id)) = (&lease.nft_contract, &lease.token_id) {
                    let client = nft_contract::NftClient::new(&env, nft);
                    client.transfer_from(
                        &env.current_contract_address(),
                        &env.current_contract_address(),
                        &lease.tenant,
                        id,
                    );
                }
                archive_lease(
                    &env,
                    lease_id,
                    lease.clone(),
                    env.current_contract_address(),
                );
                return Ok(());
            }
        }

        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn set_withdrawal_address(
        env: Env,
        lease_id: u64,
        withdrawal_address: Address,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        lease.landlord.require_auth();
        lease.withdrawal_address = Some(withdrawal_address);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn withdraw_rent(
        env: Env,
        lease_id: u64,
        _token_contract_id: Address,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        lease.landlord.require_auth();

        let _withdrawal_address = lease
            .withdrawal_address
            .clone()
            .ok_or(LeaseError::WithdrawalAddressNotSet)?;
        let _withdrawable_amount = lease.rent_paid - lease.rent_withdrawn;

        lease.rent_withdrawn += _withdrawable_amount;
        save_lease_instance(&env, lease_id, &lease);

        Ok(())
    }

    /// Terminates a lease after its `end_date` has passed and the deposit is settled.
    ///
    /// Archives the lease record and pays a small bounty to the caller from the
    /// platform fee vault to incentivise timely cleanup.
    ///
    /// # Parameters
    /// - `lease_id` – ID of the lease to terminate.
    /// - `caller`   – Must be the landlord, tenant, or admin.
    ///
    /// # Errors
    /// - [`LeaseError::LeaseNotFound`]     – No lease exists for `lease_id`.
    /// - [`LeaseError::Unauthorised`]      – Caller is not landlord, tenant, or admin.
    /// - [`LeaseError::LeaseNotExpired`]   – Current time is before `end_date`.
    /// - [`LeaseError::DepositNotSettled`] – Deposit is still `Held` or `Disputed`.
    ///
    /// # Authorization
    /// Requires `caller.require_auth()`.
    pub fn terminate_lease(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        let is_landlord = caller == lease.landlord;
        let is_tenant = caller == lease.tenant;
        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|admin| admin == caller)
            .unwrap_or(false);

        if !is_landlord && !is_tenant && !is_admin {
            return Err(LeaseError::Unauthorised);
        }
        caller.require_auth();

        if env.ledger().timestamp() < lease.end_date {
            return Err(LeaseError::LeaseNotExpired);
        }
        if lease.deposit_status == DepositStatus::Held
            || lease.deposit_status == DepositStatus::Disputed
        {
            return Err(LeaseError::DepositNotSettled);
        }

        const BOUNTY_BPS: i128 = 1_000;
        if let (Some(fee_amount), Some(fee_token), Some(fee_recipient)) = (
            env.storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::PlatformFeeAmount),
            env.storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::PlatformFeeToken),
            env.storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::PlatformFeeRecipient),
        ) {
            let bounty = fee_amount * BOUNTY_BPS / 10_000;
            if bounty > 0 {
                let token = token_contract::TokenClient::new(&env, &fee_token);
                token.transfer(&fee_recipient, &caller, &bounty);
                TerminateBountyPaid {
                    lease_id,
                    caller: caller.clone(),
                    amount: bounty,
                }
                .publish(&env);
            }
        }

        archive_lease(&env, lease_id, lease, caller);
        LeaseTerminated { lease_id }.publish(&env);
        Ok(())
    }

    pub fn execute_early_termination(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<(), LeaseError> {
        caller.require_auth();

        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Verify caller is authorized (tenant or landlord)
        if caller != lease.tenant && caller != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }

        // Check if lease is already terminated
        if lease.status == LeaseStatus::Terminated {
            return Err(LeaseError::LeaseNotTerminated);
        }

        // Initialize velocity tracker for lessor if needed
        VelocityGuard::initialize_lessor(&env, &lease.landlord)?;

        // Check velocity limits before allowing termination
        VelocityGuard::check_velocity_limits(&env, &lease.landlord)?;

        // Calculate early termination penalty
        let current_time = env.ledger().timestamp();
        let total_duration = lease.end_date - lease.start_date;
        let elapsed_time = current_time - lease.start_date;
        let remaining_time = lease.end_date - current_time;

        let penalty_amount = if let (Some(fee_bps), Some(fixed_penalty)) = 
            (lease.early_termination_fee_bps, lease.fixed_penalty) {
            // If both are configured, use the higher amount
            let percentage_penalty = if fee_bps > 0 && remaining_time > 0 {
                let remaining_value = remaining_time as i128 * lease.rent_per_sec;
                remaining_value * fee_bps as i128 / 10_000
            } else {
                0
            };
            percentage_penalty.max(fixed_penalty)
        } else if let Some(fee_bps) = lease.early_termination_fee_bps {
            // Percentage-based penalty
            if fee_bps > 0 && remaining_time > 0 {
                let remaining_value = remaining_time as i128 * lease.rent_per_sec;
                remaining_value * fee_bps as i128 / 10_000
            } else {
                0
            }
        } else if let Some(fixed_penalty) = lease.fixed_penalty {
            // Fixed penalty
            fixed_penalty
        } else {
            // No penalty configured
            0
        };

        // Cap penalty at total deposit amount
        let total_deposit = lease.security_deposit + lease.deposit_amount;
        let final_penalty = penalty_amount.min(total_deposit);

        // Calculate refund amounts
        let tenant_refund = total_deposit - final_penalty;
        let landlord_payout = final_penalty;

        // Execute transfers
        let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
        
        if tenant_refund > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &lease.tenant,
                &tenant_refund,
            );
        }

        if landlord_payout > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &lease.landlord,
                &landlord_payout,
            );
        }

        // Update lease status
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;

        // Handle NFT if present
        if let (Some(nft_contract), Some(token_id)) = 
            (lease.nft_contract.clone(), lease.token_id) {
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }

        // Record termination for velocity tracking
        VelocityGuard::record_termination(&env, &lease.landlord, lease_id)?;

        // Update portfolio size
        VelocityGuard::update_portfolio_size(&env, &lease.landlord, -1)?;

        // Archive lease
        archive_lease(&env, lease_id, lease, caller);

        // Get velocity stats for event
        let (portfolio_size, velocity_24h, _, _) = VelocityGuard::get_velocity_stats(&env, &lease.landlord)?;

        // Emit events
        LeaseTerminated { lease_id }.publish(&env);
        LeaseTerminatedWithVelocityGuard {
            lease_id,
            lessor: lease.landlord,
            tenant: lease.tenant,
            terminated_by: caller,
            timestamp: current_time,
            portfolio_size,
            velocity_24h,
        }.publish(&env);

        Ok(())
    }

    pub fn reclaim_asset(
        env: Env,
        lease_id: u64,
        caller: Address,
        reason: String,
    ) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if caller != lease.landlord && caller != lease.tenant {
            return Err(LeaseError::Unauthorised);
        }
        caller.require_auth();
        AssetReclaimed {
            id: lease_id,
            reason,
        }
        .publish(&env);
        Ok(())
    }

    pub fn conclude_lease(
        env: Env,
        lease_id: u64,
        landlord: Address,
        damage_deduction: i128,
    ) -> Result<i128, LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if landlord != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();

        if damage_deduction < 0 || damage_deduction > lease.deposit_amount {
            return Err(LeaseError::InvalidDeduction);
        }

        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        save_lease_instance(&env, lease_id, &lease);
        Ok(lease.deposit_amount - damage_deduction)
    }

    pub fn set_inspector(
        env: Env,
        lease_id: u64,
        landlord: Address,
        inspector: Address,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();
        lease.inspector = Some(inspector);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn report_maintenance_issue(
        env: Env,
        lease_id: u64,
        tenant: Address,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.tenant != tenant {
            return Err(LeaseError::Unauthorised);
        }
        tenant.require_auth();
        lease.maintenance_status = MaintenanceStatus::Reported;
        save_lease_instance(&env, lease_id, &lease);
        MaintenanceIssueReported { lease_id, tenant }.publish(&env);
        Ok(())
    }

    pub fn submit_repair_proof(
        env: Env,
        lease_id: u64,
        landlord: Address,
        proof_hash: BytesN<32>,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }

        landlord.require_auth();
        require!(
            lease.maintenance_status == MaintenanceStatus::Reported,
            "No issue reported"
        );
        lease.maintenance_status = MaintenanceStatus::Fixed;
        lease.repair_proof_hash = Some(proof_hash.clone());

        save_lease_instance(&env, lease_id, &lease);
        RepairProofSubmitted {
            lease_id,
            landlord,
            proof_hash,
        }
        .publish(&env);

        Ok(())
    }

    pub fn reclaim(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        let is_landlord = caller == lease.landlord;
        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|admin| admin == caller)
            .unwrap_or(false);

        if !is_landlord && !is_admin {
            return Err(LeaseError::Unauthorised);
        }
        caller.require_auth();

        if lease.deposit_amount > 0 {
            return Err(LeaseError::DepositNotSettled);
        }

        if let (Some(nft_contract_addr), Some(token_id)) =
            (lease.nft_contract.clone(), lease.token_id)
        {
            delete_usage_rights(&env, nft_contract_addr.clone(), token_id);
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }

        lease.status = LeaseStatus::Terminated;
        lease.active = false;

        save_lease_instance(&env, lease_id, &lease);

        AssetReclaimed {
            id: lease_id,
            reason: String::from_str(&env, "Payment stream ran dry"),
        }
        .publish(&env);

        Ok(())
    }

    pub fn verify_repair(env: Env, lease_id: u64, inspector: Address) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        match &lease.inspector {
            Some(expected) => {
                if expected != &inspector {
                    return Err(LeaseError::Unauthorised);
                }
            }
            None => return Err(LeaseError::Unauthorised),
        }
        inspector.require_auth();
        require!(
            lease.maintenance_status == MaintenanceStatus::Fixed,
            "Repair not marked as fixed"
        );

        let released = lease.withheld_rent;
        lease.cumulative_payments += released;
        lease.rent_paid += released;
        lease.withheld_rent = 0;
        lease.maintenance_status = MaintenanceStatus::Verified;

        save_lease_instance(&env, lease_id, &lease);
        MaintenanceVerified {
            lease_id,
            inspector,
            withheld_released: released,
        }
        .publish(&env);
        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address) -> Result<(), LeaseError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(LeaseError::Unauthorised);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn set_platform_fee(
        env: Env,
        admin: Address,
        fee_amount: i128,
        fee_token: Address,
        fee_recipient: Address,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeAmount, &fee_amount);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeToken, &fee_token);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeRecipient, &fee_recipient);
        Ok(())
    }

    pub fn dispute_deposit(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if caller != lease.landlord && caller != lease.tenant {
            return Err(LeaseError::Unauthorised);
        }
        caller.require_auth();

        lease.deposit_status = DepositStatus::Disputed;
        lease.status = LeaseStatus::Disputed;
        save_lease_instance(&env, lease_id, &lease);

        DepositDisputed { lease_id, caller }.publish(&env);
        Ok(())
    }

    pub fn resolve_dispute(
        env: Env,
        lease_id: u64,
        arbitrator: Address,
        damage_deduction: i128,
    ) -> Result<i128, LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        if !lease.arbitrators.contains(&arbitrator) {
            return Err(LeaseError::NotAnArbitrator);
        }
        arbitrator.require_auth();

        if damage_deduction < 0 || damage_deduction > lease.security_deposit {
            return Err(LeaseError::InvalidDeduction);
        }

        let refund_amount = lease.security_deposit - damage_deduction;

        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;

        save_lease_instance(&env, lease_id, &lease);

        Ok(refund_amount)
    }

    pub fn mutual_deposit_release(
        env: Env,
        lease_id: u64,
        lessee_pubkey: Address,
        lessor_pubkey: Address,
        return_amount: i128,
        slash_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        if lessee_pubkey != lease.tenant || lessor_pubkey != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }

        lessee_pubkey.require_auth();
        lessor_pubkey.require_auth();

        if lease.status != LeaseStatus::Active && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotFound);
        }

        let total_escrowed = lease.security_deposit + lease.deposit_amount;
        if return_amount + slash_amount != total_escrowed {
            return Err(LeaseError::InvalidReleaseMath);
        }

        if return_amount < 0 || slash_amount < 0 {
            return Err(LeaseError::InvalidReleaseMath);
        }

        let tenant_refund = return_amount;
        let landlord_payout = slash_amount;

        if tenant_refund > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.tenant,
                &tenant_refund,
            );
        }

        if landlord_payout > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.landlord,
                &landlord_payout,
            );
        }

        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;

        if let (Some(nft_contract_addr), Some(token_id)) =
            (lease.nft_contract.clone(), lease.token_id)
        {
            delete_usage_rights(&env, nft_contract_addr.clone(), token_id);
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }

        save_lease_instance(&env, lease_id, &lease);

        MutualLeaseFinalized {
            lease_id,
            return_amount,
            slash_amount,
            tenant_refund,
            landlord_payout,
        }
        .publish(&env);

        Ok(())
    }

    pub fn init_mutual_release_fb(
        env: Env,
        lease_id: u64,
        initiator_pubkey: Address,
        proposed_return_amount: i128,
        proposed_slash_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        if initiator_pubkey != lease.tenant && initiator_pubkey != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }

        initiator_pubkey.require_auth();

        if lease.status != LeaseStatus::Active && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotFound);
        }

        let total_escrowed = lease.security_deposit + lease.deposit_amount;
        if proposed_return_amount + proposed_slash_amount != total_escrowed {
            return Err(LeaseError::InvalidReleaseMath);
        }

        if proposed_return_amount < 0 || proposed_slash_amount < 0 {
            return Err(LeaseError::InvalidReleaseMath);
        }

        lease.deposit_status = DepositStatus::Disputed;
        lease.status = LeaseStatus::Disputed;

        save_lease_instance(&env, lease_id, &lease);

        DepositDisputed {
            lease_id,
            caller: initiator_pubkey,
        }
        .publish(&env);

        Ok(())
    }

    pub fn check_tenant_default(env: Env, lease_id: u64) -> Result<i128, LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        let current_time = env.ledger().timestamp();

        let elapsed_secs = current_time.saturating_sub(lease.start_date);
        let expected_rent = (elapsed_secs as i128).saturating_mul(lease.rent_per_sec);
        let unpaid_rent = expected_rent.saturating_sub(lease.rent_paid);
        let mut total_debt = if unpaid_rent > 0 { unpaid_rent } else { 0 };

        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;

            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            if seconds_late > lease.seconds_late_charged {
                let newly_accrued = seconds_late - lease.seconds_late_charged;
                lease.debt += (newly_accrued as i128) * lease.late_fee_per_sec;
                lease.seconds_late_charged = seconds_late;
            }

            let days_late = seconds_late / 86_400;
            PaymentLate {
                lease_id,
                days_late,
                current_fine: lease.debt,
            }
            .publish(&env);
        }

        total_debt += lease.debt;

        let eviction_threshold = lease.rent_amount.saturating_mul(2);

        if total_debt >= eviction_threshold {
            EvictionEligible {
                lease_id,
                tenant: lease.tenant.clone(),
                debt: total_debt,
            }
            .publish(&env);
        }

        save_lease_instance(&env, lease_id, &lease);
        Ok(total_debt)
    }

    pub fn add_authorized_payer(
        env: Env,
        lease_id: u64,
        landlord: Address,
        roommate: Address,
    ) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();

        let key = DataKey::AuthorizedPayer(lease_id, roommate.clone());
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);

        RoommateAdded { lease_id, roommate }.publish(&env);
        Ok(())
    }

    pub fn get_roommate_balance(env: Env, lease_id: u64, roommate: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::RoommateBalance(lease_id, roommate))
            .unwrap_or(0)
    }

    pub fn set_terms_hash(env: Env, admin: Address, hash: BytesN<32>) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::TermsHash, &hash);
        TermsHashUpdated {
            new_terms_hash: hash,
        }
        .publish(&env);
        Ok(())
    }

    pub fn upgrade(
        env: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        expected_terms_hash: BytesN<32>,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        if let Some(current_hash) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&DataKey::TermsHash)
        {
            if current_hash != expected_terms_hash {
                return Err(LeaseError::UpgradeNotAllowed);
            }
        }

        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    pub fn whitelist_oracle(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::WhitelistedOracle(oracle_pubkey), &true);
        Ok(())
    }

    pub fn remove_oracle(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        env.storage()
            .instance()
            .remove(&DataKey::WhitelistedOracle(oracle_pubkey));
        Ok(())
    }

    fn is_oracle_whitelisted(env: &Env, oracle_pubkey: &BytesN<32>) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::WhitelistedOracle(oracle_pubkey.clone()))
    }

    fn get_oracle_config(env: &Env, oracle_pubkey: &BytesN<32>) -> Option<OracleConfig> {
        env.storage()
            .instance()
            .get(&DataKey::OracleConfig(oracle_pubkey.clone()))
    }

    fn set_oracle_config(env: &Env, oracle_pubkey: &BytesN<32>, config: &OracleConfig) {
        env.storage()
            .instance()
            .set(&DataKey::OracleConfig(oracle_pubkey.clone()), config);
    }

    fn get_fallback_hierarchy(env: &Env) -> Option<FallbackHierarchy> {
        env.storage()
            .instance()
            .get(&DataKey::FallbackHierarchy)
    }

    fn set_fallback_hierarchy(env: &Env, hierarchy: &FallbackHierarchy) {
        env.storage()
            .instance()
            .set(&DataKey::FallbackHierarchy, hierarchy);
    }

    fn staleness_check(env: &Env, payload_timestamp: u64) -> Result<bool, LeaseError> {
        let current_time = env.ledger().timestamp();
        
        // Check if timestamp is from the future
        if payload_timestamp > current_time {
            return Err(LeaseError::OracleStale);
        }
        
        // Check if timestamp is older than 48 hours
        let age = current_time.saturating_sub(payload_timestamp);
        if age > STALENESS_THRESHOLD {
            return Err(LeaseError::OracleStale);
        }
        
        Ok(true)
    }

    fn demote_oracle(
        env: &Env, 
        oracle_pubkey: &BytesN<32>, 
        reason: String
    ) -> Result<(), LeaseError> {
        let mut config = Self::get_oracle_config(env, oracle_pubkey)
            .ok_or(LeaseError::OracleNotWhitelisted)?;
        
        if config.status == OracleStatus::Demoted {
            return Ok(()); // Already demoted
        }
        
        config.status = OracleStatus::Demoted;
        config.demotion_timestamp = Some(env.ledger().timestamp());
        config.failure_count += 1;
        
        Self::set_oracle_config(env, oracle_pubkey, &config);
        
        OracleDemoted {
            oracle_pubkey: oracle_pubkey.clone(),
            reason,
            demotion_timestamp: env.ledger().timestamp(),
            failure_count: config.failure_count,
        }
        .publish(env);
        
        Ok(())
    }

    fn activate_fallback_hierarchy(
        env: &Env,
        reason: String
    ) -> Result<(), LeaseError> {
        let mut hierarchy = Self::get_fallback_hierarchy(env)
            .ok_or(LeaseError::FallbackHierarchyNotConfigured)?;
        
        if hierarchy.hierarchy_active {
            return Ok(()); // Already active
        }
        
        hierarchy.hierarchy_active = true;
        hierarchy.last_demotion_time = Some(env.ledger().timestamp());
        
        Self::set_fallback_hierarchy(env, &hierarchy);
        
        FallbackHierarchyActivated {
            primary_oracle: hierarchy.primary_oracle.clone(),
            backup_oracle: hierarchy.backup_oracle.clone(),
            activation_timestamp: env.ledger().timestamp(),
            reason,
        }
        .publish(env);
        
        Ok(())
    }

    fn trigger_dao_arbitration(
        env: &Env,
        lease_id: u64,
        reason: String
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        lease.status = LeaseStatus::DaoArbitration;
        save_lease_instance(env, lease_id, &lease);
        
        DaoArbitrationTriggered {
            lease_id,
            trigger_timestamp: env.ledger().timestamp(),
            reason,
        }
        .publish(env);
        
        Ok(())
    }

    fn check_oracle_availability(
        env: &Env,
        oracle_pubkey: &BytesN<32>
    ) -> Result<bool, LeaseError> {
        let config = Self::get_oracle_config(env, oracle_pubkey)
            .ok_or(LeaseError::OracleNotWhitelisted)?;
        
        match config.status {
            OracleStatus::Active => Ok(true),
            OracleStatus::Demoted => {
                // Check if enough time has passed to allow retry
                if let Some(demotion_time) = config.demotion_timestamp {
                    let current_time = env.ledger().timestamp();
                    let time_since_demotion = current_time.saturating_sub(demotion_time);
                    
                    // Allow retry after 24 hours if failure count is below threshold
                    if time_since_demotion > 24 * 60 * 60 && config.failure_count < MAX_ORACLE_FAILURES {
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            },
            OracleStatus::Failed => Ok(false),
        }
    }

    fn is_yield_protocol_whitelisted(env: &Env, protocol: &Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::WhitelistedYieldProtocol(protocol.clone()))
    }

    fn get_liquidity_buffer(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::LiquidityBuffer)
            .unwrap_or(0)
    }

    fn set_liquidity_buffer(env: &Env, amount: i128) {
        env.storage()
            .instance()
            .set(&DataKey::LiquidityBuffer, &amount);
    }

    fn calculate_yield_distribution(total_yield: i128) -> (i128, i128, i128) {
        const LESSEE_BPS: u32 = 5000;
        const LESSOR_BPS: u32 = 3000;
        const DAO_BPS: u32 = 2000;

        let lessee_share = total_yield.saturating_mul(LESSEE_BPS as i128) / 10_000;
        let lessor_share = total_yield.saturating_mul(LESSOR_BPS as i128) / 10_000;
        let dao_share = total_yield.saturating_mul(DAO_BPS as i128) / 10_000;

        (lessee_share, lessor_share, dao_share)
    }

    fn verify_liquidity_buffer(env: &Env, required_amount: i128) -> Result<(), LeaseError> {
        let current_buffer = Self::get_liquidity_buffer(env);
        if current_buffer < required_amount {
            return Err(LeaseError::InsufficientLiquidityBuffer);
        }
        Ok(())
    }

    pub fn dao_approve_resume(
        env: Env,
        dao_member: Address,
        lessor: Address,
        request_id: u64,
    ) -> Result<(), LeaseError> {
        dao_member.require_auth();
        
        VelocityGuard::dao_approve_resume(&env, &dao_member, &lessor, request_id)
    }

    fn verify_ed25519_signature(
        env: &Env,
        pubkey: &BytesN<32>,
        message: &soroban_sdk::Bytes,
        signature: &BytesN<64>,
    ) {
        env.crypto().ed25519_verify(pubkey, message, signature);
    }

    fn calculate_penalty_percentage(severity: DamageSeverity) -> u32 {
        match severity {
            DamageSeverity::NormalWearAndTear => 0,
            DamageSeverity::Minor => 10,
            DamageSeverity::Moderate => 25,
            DamageSeverity::Major => 50,
            DamageSeverity::Severe => 75,
            DamageSeverity::Catastrophic => 100,
        }
    }

    fn get_oracle_nonce(env: &Env, oracle_pubkey: &BytesN<32>) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::OracleNonce(oracle_pubkey.clone(), 0))
            .unwrap_or(0)
    }

    fn set_oracle_nonce(env: &Env, oracle_pubkey: &BytesN<32>, nonce: u64) {
        env.storage()
            .persistent()
            .set(&DataKey::OracleNonce(oracle_pubkey.clone(), 0), &nonce);
        env.storage().persistent().extend_ttl(
            &DataKey::OracleNonce(oracle_pubkey.clone(), 0),
            YEAR_IN_LEDGERS,
            YEAR_IN_LEDGERS,
        );
    }

    fn is_tenant_flagged(env: &Env, lease_id: u64) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::TenantFlag(lease_id))
    }

    fn flag_tenant(env: &Env, lease_id: u64, tenant: Address, reason: String) {
        env.storage()
            .persistent()
            .set(&DataKey::TenantFlag(lease_id), &true);
        env.storage().persistent().extend_ttl(
            &DataKey::TenantFlag(lease_id),
            YEAR_IN_LEDGERS,
            YEAR_IN_LEDGERS,
        );

        TenantFlagged {
            lease_id,
            tenant,
            reason,
        }
        .publish(env);
    }

    pub fn execute_deposit_slash(env: Env, payload: OraclePayload) -> Result<(), LeaseError> {
        let current_time = env.ledger().timestamp();
        
        // First, check staleness
        Self::staleness_check(&env, payload.timestamp)?;
        
        // Get fallback hierarchy configuration
        let hierarchy = Self::get_fallback_hierarchy(&env);
        
        // Check if the oracle is whitelisted and available
        let mut oracle_available = Self::is_oracle_whitelisted(&env, &payload.oracle_pubkey);
        
        if oracle_available {
            oracle_available = Self::check_oracle_availability(&env, &payload.oracle_pubkey)?;
        }
        
        // If primary oracle is not available, try fallback logic
        if !oracle_available {
            if let Some(hierarchy_config) = hierarchy {
                // Check if this is the primary oracle that failed
                if payload.oracle_pubkey == hierarchy_config.primary_oracle {
                    // Demote primary oracle
                    Self::demote_oracle(&env, &payload.oracle_pubkey, 
                        String::from_str(&env, "Primary oracle stale or unavailable"))?;
                    
                    // Activate fallback hierarchy
                    Self::activate_fallback_hierarchy(&env, 
                        String::from_str(&env, "Primary oracle demoted due to staleness"))?;
                    
                    return Err(LeaseError::OracleStale);
                }
                // Check if this is the backup oracle that failed
                else if payload.oracle_pubkey == hierarchy_config.backup_oracle {
                    // Check if this is a prolonged backup failure (> 7 days)
                    if let Some(last_demotion) = hierarchy_config.last_demotion_time {
                        let time_since_demotion = current_time.saturating_sub(last_demotion);
                        if time_since_demotion > BACKUP_FAILURE_THRESHOLD {
                            // Trigger DAO arbitration for affected leases
                            Self::trigger_dao_arbitration(&env, payload.lease_id,
                                String::from_str(&env, "Backup oracle failed for 7+ days"))?;
                            return Err(LeaseError::OracleUnavailable);
                        }
                    }
                    
                    // Demote backup oracle
                    Self::demote_oracle(&env, &payload.oracle_pubkey,
                        String::from_str(&env, "Backup oracle failed"))?;
                    
                    return Err(LeaseError::OracleUnavailable);
                }
            }
            
            return Err(LeaseError::OracleNotWhitelisted);
        }
        
        // Security validation: prevent oracle bypass attempts
        if let Some(hierarchy_config) = hierarchy {
            if hierarchy_config.hierarchy_active {
                // If hierarchy is active, only allow backup oracle
                if payload.oracle_pubkey != hierarchy_config.backup_oracle {
                    return Err(LeaseError::OracleBypassAttempt);
                }
            }
        }
        
        // Standard oracle validation
        let stored_nonce = Self::get_oracle_nonce(&env, &payload.oracle_pubkey);
        if payload.nonce <= stored_nonce {
            return Err(LeaseError::InvalidNonce);
        }

        // Additional timestamp validation (redundant with staleness_check but kept for security)
        if payload.timestamp > current_time || current_time - payload.timestamp > 86400 {
            return Err(LeaseError::InvalidSignature);
        }

        let message_data = soroban_sdk::Bytes::from_slice(&env, &payload.lease_id.to_be_bytes());

        Self::verify_ed25519_signature(
            &env,
            &payload.oracle_pubkey,
            &message_data,
            &payload.signature,
        );

        // Update oracle success timestamp
        if let Some(mut config) = Self::get_oracle_config(&env, &payload.oracle_pubkey) {
            config.last_successful_timestamp = current_time;
            if config.status == OracleStatus::Demoted && config.failure_count < MAX_ORACLE_FAILURES {
                config.status = OracleStatus::Active; // Reinstate on success
                config.failure_count = 0;
                config.demotion_timestamp = None;
            }
            Self::set_oracle_config(&env, &payload.oracle_pubkey, &config);
        }

        Self::set_oracle_nonce(&env, &payload.oracle_pubkey, payload.nonce);

        let mut lease = load_lease_instance_by_id(&env, payload.lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Check if lease is in DAO arbitration state
        if lease.status == LeaseStatus::DaoArbitration {
            return Err(LeaseError::DaoArbitrationNotEnabled);
        }

        if lease.status != LeaseStatus::Terminated && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotTerminated);
        }

        if lease.deposit_status == DepositStatus::Settled {
            return Err(LeaseError::DepositAlreadySettled);
        }

        // Initialize velocity tracker for lessor if needed
        VelocityGuard::initialize_lessor(&env, &lease.landlord)?;

        // Check velocity limits before allowing deposit slash
        VelocityGuard::check_velocity_limits(&env, &lease.landlord)?;

        let total_deposit = lease.security_deposit + lease.deposit_amount;
        let penalty_percentage = Self::calculate_penalty_percentage(payload.damage_severity);
        let penalty_amount = if penalty_percentage == 0 {
            0
        } else {
            total_deposit.saturating_mul(penalty_percentage as i128) / 100
        };

        let tenant_refund = total_deposit.saturating_sub(penalty_amount);
        let landlord_payout = penalty_amount;

        // Issue #117: Check if this is a massive slash requiring multi-sig veto
        if landlord_payout >= MASSIVE_SLASH_THRESHOLD {
            // Check if security council is initialized
            if env.storage().instance().get(&DataKey::SecurityCouncil).is_some() {
                // Create pending slash with timelock
                let current_time = env.ledger().timestamp();
                let pending_slash = PendingSlashVeto {
                    lease_id: payload.lease_id,
                    slash_amount: penalty_amount,
                    tenant_refund,
                    landlord_payout,
                    oracle_payload: payload.clone(),
                    proposed_at: current_time,
                    timelock_end: current_time + VETO_TIMELOCK_PERIOD,
                    veto_votes_for: 0,
                    veto_votes_against: 0,
                    executed: false,
                    vetoed: false,
                };

                env.storage()
                    .instance()
                    .set(&DataKey::PendingSlash(payload.lease_id), &pending_slash);

                // Don't execute immediately - wait for veto period
                return Err(LeaseError::PendingVeto);
            }
        }

        // Standard slash execution (no veto or council not initialized)

        if payload.damage_severity as u32 >= DamageSeverity::Severe as u32
            && penalty_amount >= total_deposit
        {
            Self::flag_tenant(
                &env,
                payload.lease_id,
                lease.tenant.clone(),
                String::from_str(&env, "Severe damage exceeding deposit value"),
            );
        }

        if tenant_refund > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.tenant,
                &tenant_refund,
            );
        }

        if landlord_payout > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.landlord,
                &landlord_payout,
            );
        }

        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;
        save_lease_instance(&env, payload.lease_id, &lease);

        // Record deposit slash for velocity tracking
        VelocityGuard::record_termination(&env, &lease.landlord, payload.lease_id)?;

        // Update portfolio size
        VelocityGuard::update_portfolio_size(&env, &lease.landlord, -1)?;

        if let (Some(nft_contract_addr), Some(token_id)) =
            (lease.nft_contract.clone(), lease.token_id)
        {
            delete_usage_rights(&env, nft_contract_addr.clone(), token_id);
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }

        // Get velocity stats for event
        let (portfolio_size, velocity_24h, _, _) = VelocityGuard::get_velocity_stats(&env, &lease.landlord)?;

        DepositSlashed {
            lease_id: payload.lease_id,
            oracle_pubkey: payload.oracle_pubkey.clone(),
            damage_code: payload.damage_severity as u32,
            deducted_amount: penalty_amount,
            tenant_refund,
            landlord_payout,
        }
        .publish(&env);

        Ok(())
    }

    pub fn set_fallback_hierarchy(
        env: Env,
        admin: Address,
        primary_oracle: BytesN<32>,
        backup_oracle: BytesN<32>,
        dao_arbitration_enabled: bool,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        
        // Validate both oracles are whitelisted
        if !Self::is_oracle_whitelisted(&env, &primary_oracle) {
            return Err(LeaseError::OracleNotWhitelisted);
        }
        if !Self::is_oracle_whitelisted(&env, &backup_oracle) {
            return Err(LeaseError::OracleNotWhitelisted);
        }
        
        // Ensure oracles are different
        if primary_oracle == backup_oracle {
            return Err(LeaseError::InvalidReleaseMath); // Reuse error for validation
        }
        
        // Initialize oracle configurations
        let primary_config = OracleConfig {
            pubkey: primary_oracle.clone(),
            tier: OracleTier::Primary,
            status: OracleStatus::Active,
            last_successful_timestamp: env.ledger().timestamp(),
            demotion_timestamp: None,
            failure_count: 0,
        };
        
        let backup_config = OracleConfig {
            pubkey: backup_oracle.clone(),
            tier: OracleTier::Backup,
            status: OracleStatus::Active,
            last_successful_timestamp: env.ledger().timestamp(),
            demotion_timestamp: None,
            failure_count: 0,
        };
        
        let hierarchy = FallbackHierarchy {
            primary_oracle,
            backup_oracle,
            dao_arbitration_enabled,
            hierarchy_active: false,
            last_demotion_time: None,
        };
        
        Self::set_oracle_config(&env, &primary_config.pubkey, &primary_config);
        Self::set_oracle_config(&env, &backup_config.pubkey, &backup_config);
        Self::set_fallback_hierarchy(&env, &hierarchy);
        
        Ok(())
    }

    pub fn get_oracle_status(
        env: Env,
        oracle_pubkey: BytesN<32>,
    ) -> Result<OracleConfig, LeaseError> {
        let config = Self::get_oracle_config(&env, &oracle_pubkey)
            .ok_or(LeaseError::OracleNotWhitelisted)?;
        Ok(config)
    }

    pub fn get_hierarchy_status(env: Env) -> Result<FallbackHierarchy, LeaseError> {
        let hierarchy = Self::get_fallback_hierarchy(&env)
            .ok_or(LeaseError::FallbackHierarchyNotConfigured)?;
        Ok(hierarchy)
    }

    pub fn reset_oracle_status(
        env: Env,
        admin: Address,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();
        
        let mut config = Self::get_oracle_config(&env, &oracle_pubkey)
            .ok_or(LeaseError::OracleNotWhitelisted)?;
        
        config.status = OracleStatus::Active;
        config.failure_count = 0;
        config.demotion_timestamp = None;
        config.last_successful_timestamp = env.ledger().timestamp();
        
        Self::set_oracle_config(&env, &oracle_pubkey, &config);
        
        Ok(())
    }

    pub fn set_liquidity_buffer_amount(
        env: Env,
        admin: Address,
        buffer_amount: i128,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        Self::set_liquidity_buffer(&env, buffer_amount);
        Ok(())
    }

    pub fn deploy_escrow_to_yield(
        env: Env,
        lease_id: u64,
        yield_protocol: Address,
        deploy_amount: i128,
        max_slippage_bps: u32,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        if !Self::is_yield_protocol_whitelisted(&env, &yield_protocol) {
            return Err(LeaseError::YieldProtocolNotWhitelisted);
        }

        if lease.security_deposit < deploy_amount {
            return Err(LeaseError::InvalidDeduction);
        }

        Self::verify_liquidity_buffer(&env, deploy_amount)?;

        let yield_client = yield_protocol::YieldClient::new(&env, &yield_protocol);
        let lp_tokens = yield_client.deposit(&env.current_contract_address(), &deploy_amount);

        if lp_tokens
            < deploy_amount.saturating_mul(10_000i128 - max_slippage_bps as i128) / 10_000i128
        {
            return Err(LeaseError::SlippageExceeded);
        }

        let deployment = YieldDeployment {
            lease_id,
            principal_amount: deploy_amount,
            yield_protocol: yield_protocol.clone(),
            deployment_timestamp: env.ledger().timestamp(),
            lp_tokens,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::YieldDeployment(lease_id), &deployment);
        env.storage().persistent().extend_ttl(
            &DataKey::YieldDeployment(lease_id),
            YEAR_IN_LEDGERS,
            YEAR_IN_LEDGERS,
        );

        lease.security_deposit -= deploy_amount;
        save_lease_instance(&env, lease_id, &lease);

        let current_buffer = Self::get_liquidity_buffer(&env);
        Self::set_liquidity_buffer(&env, current_buffer - deploy_amount);

        Ok(())
    }

    pub fn harvest_yield(env: Env, lease_id: u64) -> Result<(), LeaseError> {
        let deployment: YieldDeployment = env
            .storage()
            .persistent()
            .get(&DataKey::YieldDeployment(lease_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if !deployment.active {
            return Err(LeaseError::LeaseNotFound);
        }

        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        let yield_client = yield_protocol::YieldClient::new(&env, &deployment.yield_protocol);
        let total_yield = yield_client.claim_rewards(&env.current_contract_address());

        if total_yield <= 0 {
            return Err(LeaseError::YieldUnderflow);
        }

        let (lessee_share, lessor_share, dao_share) =
            Self::calculate_yield_distribution(total_yield);

        let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);

        if lessee_share > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &lease.tenant,
                &lessee_share,
            );
        }

        if lessor_share > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &lease.landlord,
                &lessor_share,
            );
        }

        if dao_share > 0 {
            if let Some(dao_address) = env.storage().instance().get(&DataKey::PlatformFeeRecipient)
            {
                token_client.transfer(&env.current_contract_address(), &dao_address, &dao_share);
            }
        }

        let accumulated_yield: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::YieldAccumulated(lease_id))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::YieldAccumulated(lease_id),
            &(accumulated_yield + total_yield),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::YieldAccumulated(lease_id),
            YEAR_IN_LEDGERS,
            YEAR_IN_LEDGERS,
        );

        EscrowYieldHarvested {
            lease_id,
            total_yield,
            lessee_share,
            lessor_share,
            dao_share,
            yield_protocol: deployment.yield_protocol.clone(),
            harvest_timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        Ok(())
    }

    pub fn withdraw_from_yield(
        env: Env,
        lease_id: u64,
        max_slippage_bps: u32,
    ) -> Result<(), LeaseError> {
        let deployment: YieldDeployment = env
            .storage()
            .persistent()
            .get(&DataKey::YieldDeployment(lease_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if !deployment.active {
            return Err(LeaseError::LeaseNotFound);
        }

        let yield_client = yield_protocol::YieldClient::new(&env, &deployment.yield_protocol);
        let withdrawn_amount =
            yield_client.withdraw(&env.current_contract_address(), &deployment.lp_tokens);

        if withdrawn_amount < deployment.principal_amount {
            return Err(LeaseError::YieldUnderflow);
        }

        let min_expected = deployment
            .principal_amount
            .saturating_mul(10_000i128 - max_slippage_bps as i128)
            / 10_000i128;
        if withdrawn_amount < min_expected {
            return Err(LeaseError::SlippageExceeded);
        }

        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        lease.security_deposit += withdrawn_amount;
        save_lease_instance(&env, lease_id, &lease);

        let mut updated_deployment = deployment.clone();
        updated_deployment.active = false;
        updated_deployment.lp_tokens = 0;
        env.storage()
            .persistent()
            .set(&DataKey::YieldDeployment(lease_id), &updated_deployment);

        let current_buffer = Self::get_liquidity_buffer(&env);
        Self::set_liquidity_buffer(&env, current_buffer + withdrawn_amount);

        Ok(())
    }

    pub fn get_yield_deployment(env: Env, lease_id: u64) -> Result<YieldDeployment, LeaseError> {
        env.storage()
            .persistent()
            .get(&DataKey::YieldDeployment(lease_id))
            .ok_or(LeaseError::LeaseNotFound)
    }

    pub fn get_accumulated_yield(env: Env, lease_id: u64) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::YieldAccumulated(lease_id))
            .unwrap_or(0)
    }

    // ========================================================================
    // Issue #117: Multi-Sig Veto on Massive Deposit Slashing
    // ========================================================================

    pub fn initialize_security_council(
        env: Env,
        admin: Address,
        members: soroban_sdk::Vec<SecurityCouncilMember>,
        veto_threshold_bps: u32,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        let mut total_voting_power: u32 = 0;
        for member in members.iter() {
            total_voting_power += member.voting_power;
        }

        let council = SecurityCouncil {
            members,
            veto_threshold_bps,
            total_voting_power,
        };

        env.storage()
            .instance()
            .set(&DataKey::SecurityCouncil, &council);

        Ok(())
    }

    pub fn add_council_member(
        env: Env,
        admin: Address,
        member: SecurityCouncilMember,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        let mut council: SecurityCouncil = env
            .storage()
            .instance()
            .get(&DataKey::SecurityCouncil)
            .ok_or(LeaseError::Unauthorized)?;

        let mut members = council.members;
        members.push_back(member);
        council.members = members;
        council.total_voting_power += member.voting_power;

        env.storage()
            .instance()
            .set(&DataKey::SecurityCouncil, &council);

        Ok(())
    }

    pub fn veto_slash_vote(
        env: Env,
        council_member: Address,
        lease_id: u64,
        vote_for_veto: bool,
    ) -> Result<(), LeaseError> {
        // Verify caller is a council member
        let council: SecurityCouncil = env
            .storage()
            .instance()
            .get(&DataKey::SecurityCouncil)
            .ok_or(LeaseError::Unauthorized)?;

        let mut member_voting_power: u32 = 0;
        let mut is_member = false;

        for member in council.members.iter() {
            if member.address == council_member && member.active {
                member_voting_power = member.voting_power;
                is_member = true;
                break;
            }
        }

        if !is_member {
            return Err(LeaseError::Unauthorized);
        }

        council_member.require_auth();

        // Get pending slash
        let mut pending_slash: PendingSlashVeto = env
            .storage()
            .instance()
            .get(&DataKey::PendingSlash(lease_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if pending_slash.executed || pending_slash.vetoed {
            return Err(LeaseError::InvalidState);
        }

        // Check if already voted
        let vote_key = DataKey::VetoVote(lease_id, council_member.clone());
        if env.storage().instance().get(&vote_key).is_some() {
            return Err(LeaseError::AlreadyVoted);
        }

        // Record vote
        if vote_for_veto {
            pending_slash.veto_votes_for += member_voting_power;
        } else {
            pending_slash.veto_votes_against += member_voting_power;
        }

        env.storage()
            .instance()
            .set(&DataKey::PendingSlash(lease_id), &pending_slash);
        env.storage().instance().set(&vote_key, &true);

        Ok(())
    }

    pub fn execute_pending_slash(env: Env, lease_id: u64) -> Result<(), LeaseError> {
        let pending_slash: PendingSlashVeto = env
            .storage()
            .instance()
            .get(&DataKey::PendingSlash(lease_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if pending_slash.executed || pending_slash.vetoed {
            return Err(LeaseError::InvalidState);
        }

        let current_time = env.ledger().timestamp();

        // Check if timelock has expired
        if current_time < pending_slash.timelock_end {
            return Err(LeaseError::TimelockNotExpired);
        }

        // Check if veto was successful
        let council: SecurityCouncil = env
            .storage()
            .instance()
            .get(&DataKey::SecurityCouncil)
            .ok_or(LeaseError::Unauthorized)?;

        let veto_threshold = (council.total_voting_power as i128
            * council.veto_threshold_bps as i128)
            / 10000;

        if pending_slash.veto_votes_for as i128 >= veto_threshold {
            // Veto succeeded - cancel the slash
            let mut updated_slash = pending_slash.clone();
            updated_slash.vetoed = true;
            updated_slash.executed = true;
            env.storage()
                .instance()
                .set(&DataKey::PendingSlash(lease_id), &updated_slash);

            // Return full deposit to tenant
            let token_client =
                token_contract::TokenClient::new(&env, &pending_slash.oracle_payload.lease_id.into());
            // Note: Actual token transfer would need the correct token address from lease

            return Ok(());
        }

        // Veto failed - execute the original slash
        let mut updated_slash = pending_slash.clone();
        updated_slash.executed = true;
        env.storage()
            .instance()
            .set(&DataKey::PendingSlash(lease_id), &updated_slash);

        // Execute the original slash logic
        Self::execute_deposit_slash(env, pending_slash.oracle_payload)
    }

    // ========================================================================
    // Issue #118: DAO-Governed Dynamic Protocol Fee Updates
    // ========================================================================

    pub fn initialize_protocol_fee_config(
        env: Env,
        admin: Address,
        config: ProtocolFeeConfig,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        // Validate configuration
        if config.current_fee_bps > config.max_fee_bps {
            return Err(LeaseError::InvalidParameters);
        }

        env.storage()
            .instance()
            .set(&DataKey::ProtocolFeeConfig, &config);

        Ok(())
    }

    pub fn propose_fee_update(
        env: Env,
        proposer: Address,
        new_fee_bps: u32,
    ) -> Result<(), LeaseError> {
        proposer.require_auth();

        let config: ProtocolFeeConfig = env
            .storage()
            .instance()
            .get(&DataKey::ProtocolFeeConfig)
            .ok_or(LeaseError::Unauthorized)?;

        // Validate fee is within bounds
        if new_fee_bps < config.min_fee_bps || new_fee_bps > config.max_fee_bps {
            return Err(LeaseError::InvalidParameters);
        }

        // Validate max increase limit
        let current_fee = config.current_fee_bps;
        if new_fee_bps > current_fee {
            let increase = new_fee_bps - current_fee;
            if increase > config.max_increase_bps {
                return Err(LeaseError::InvalidParameters);
            }
        }

        let current_time = env.ledger().timestamp();
        let execution_time = current_time + config.update_timelock;

        let pending_update = PendingFeeUpdate {
            proposed_fee_bps: new_fee_bps,
            proposed_by: proposer,
            proposed_at: current_time,
            execution_time,
            votes_for: 0,
            votes_against: 0,
            executed: false,
        };

        env.storage()
            .instance()
            .set(&DataKey::PendingFeeUpdate, &pending_update);

        Ok(())
    }

    pub fn vote_on_fee_update(
        env: Env,
        voter: Address,
        vote_for: bool,
    ) -> Result<(), LeaseError> {
        voter.require_auth();

        let mut pending_update: PendingFeeUpdate = env
            .storage()
            .instance()
            .get(&DataKey::PendingFeeUpdate)
            .ok_or(LeaseError::Unauthorized)?;

        if pending_update.executed {
            return Err(LeaseError::InvalidState);
        }

        // Simple vote counting (can be enhanced with token-weighted voting)
        if vote_for {
            pending_update.votes_for += 1;
        } else {
            pending_update.votes_against += 1;
        }

        env.storage()
            .instance()
            .set(&DataKey::PendingFeeUpdate, &pending_update);

        Ok(())
    }

    pub fn execute_fee_update(env: Env) -> Result<(), LeaseError> {
        let mut pending_update: PendingFeeUpdate = env
            .storage()
            .instance()
            .get(&DataKey::PendingFeeUpdate)
            .ok_or(LeaseError::Unauthorized)?;

        if pending_update.executed {
            return Err(LeaseError::InvalidState);
        }

        let current_time = env.ledger().timestamp();

        // Check timelock
        if current_time < pending_update.execution_time {
            return Err(LeaseError::TimelockNotExpired);
        }

        // Check if proposal passed (simple majority)
        if pending_update.votes_for <= pending_update.votes_against {
            return Err(LeaseError::ProposalRejected);
        }

        // Update fee configuration
        let mut config: ProtocolFeeConfig = env
            .storage()
            .instance()
            .get(&DataKey::ProtocolFeeConfig)
            .ok_or(LeaseError::Unauthorized)?;

        config.current_fee_bps = pending_update.proposed_fee_bps;
        env.storage()
            .instance()
            .set(&DataKey::ProtocolFeeConfig, &config);

        // Mark as executed
        pending_update.executed = true;
        env.storage()
            .instance()
            .set(&DataKey::PendingFeeUpdate, &pending_update);

        Ok(())
    }

    pub fn get_protocol_fee_config(env: Env) -> Result<ProtocolFeeConfig, LeaseError> {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolFeeConfig)
            .ok_or(LeaseError::Unauthorized)
    }

    // ========================================================================
    // Issue #119: Quadratic Voting for Treasury Yield Allocation
    // ========================================================================

    pub fn create_governance_round(
        env: Env,
        admin: Address,
        round_id: u64,
        total_treasury_yield: i128,
        allocation_options: soroban_sdk::Vec<AllocationOption>,
    ) -> Result<(), LeaseError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin {
            return Err(LeaseError::Unauthorised);
        }
        admin.require_auth();

        let current_time = env.ledger().timestamp();
        let snapshot_timestamp = current_time - FLASH_LOAN_PROTECTION_BUFFER;

        let round = GovernanceRound {
            round_id,
            start_time: current_time,
            end_time: current_time + GOVERNANCE_ROUND_DURATION,
            total_treasury_yield,
            allocation_options,
            active: true,
            snapshot_timestamp,
        };

        env.storage()
            .instance()
            .set(&DataKey::GovernanceRound(round_id), &round);

        Ok(())
    }

    pub fn cast_treasury_vote(
        env: Env,
        voter: Address,
        round_id: u64,
        option_id: u32,
        tokens_committed: i128,
    ) -> Result<(), LeaseError> {
        voter.require_auth();

        if tokens_committed <= 0 {
            return Err(LeaseError::InvalidParameters);
        }

        let round: GovernanceRound = env
            .storage()
            .instance()
            .get(&DataKey::GovernanceRound(round_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if !round.active {
            return Err(LeaseError::InvalidState);
        }

        let current_time = env.ledger().timestamp();
        if current_time > round.end_time {
            return Err(LeaseError::GovernanceRoundEnded);
        }

        // Calculate quadratic voting power: sqrt(tokens_committed)
        let voting_power = Self::integer_sqrt(tokens_committed);

        // Record the vote
        let vote = TreasuryVote {
            round_id,
            voter: voter.clone(),
            option_id,
            tokens_committed,
            voting_power,
            voted_at: current_time,
        };

        env.storage()
            .instance()
            .set(&DataKey::TreasuryVote(round_id, voter), &vote);

        // Update allocation option totals
        let mut options = round.allocation_options;
        for i in 0..options.len() {
            let mut option = options.get(i).unwrap();
            if option.option_id == option_id {
                option.total_quadratic_votes += voting_power;
                options.set(i, option);
                break;
            }
        }

        let mut updated_round = round;
        updated_round.allocation_options = options;
        env.storage()
            .instance()
            .set(&DataKey::GovernanceRound(round_id), &updated_round);

        Ok(())
    }

    pub fn finalize_governance_round(
        env: Env,
        round_id: u64,
    ) -> Result<soroban_sdk::Vec<AllocationOption>, LeaseError> {
        let mut round: GovernanceRound = env
            .storage()
            .instance()
            .get(&DataKey::GovernanceRound(round_id))
            .ok_or(LeaseError::LeaseNotFound)?;

        if !round.active {
            return Err(LeaseError::InvalidState);
        }

        let current_time = env.ledger().timestamp();
        if current_time < round.end_time {
            return Err(LeaseError::GovernanceRoundActive);
        }

        round.active = false;
        env.storage()
            .instance()
            .set(&DataKey::GovernanceRound(round_id), &round);

        // Calculate total quadratic votes
        let mut total_quadratic_votes: i128 = 0;
        for option in round.allocation_options.iter() {
            total_quadratic_votes += option.total_quadratic_votes;
        }

        // Distribute treasury yield proportionally
        let options = round.allocation_options;
        // Note: Actual token transfers would be executed here based on allocations

        Ok(options)
    }

    fn integer_sqrt(n: i128) -> i128 {
        if n <= 0 {
            return 0;
        }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
    }

    // ========================================================================
    // Issue #124: Highly Optimized get_active_leases Read-Only Query
    // ========================================================================

    /// Returns a summary of all currently active or pending leases.
    ///
    /// Read-only; performs no state mutations. Uses the `ActiveLeasesIndex` for
    /// O(n) iteration rather than a full storage scan.
    ///
    /// # Returns
    /// A [`Vec<ActiveLeaseSummary>`] containing one entry per active/pending lease.
    pub fn get_active_leases(env: Env) -> soroban_sdk::Vec<ActiveLeaseSummary> {
        // This is a read-only function - no state mutations
        // Returns comprehensive lease data for frontend rendering

        let mut active_leases = soroban_sdk::Vec::new(&env);

        // Iterate through lease instances (in production, this would use an index)
        // For optimization, we maintain an ActiveLeasesIndex
        let lease_ids: soroban_sdk::Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::ActiveLeasesIndex)
            .unwrap_or(soroban_sdk::Vec::new(&env));

        for lease_id in lease_ids.iter() {
            if let Ok(lease) = Self::get_lease_instance(env.clone(), lease_id) {
                // Only return active leases
                if lease.active
                    && (lease.status == LeaseStatus::Active
                        || lease.status == LeaseStatus::Pending)
                {
                    let summary = ActiveLeaseSummary {
                        lease_id,
                        landlord: lease.landlord,
                        tenant: lease.tenant,
                        rent_amount: lease.rent_amount,
                        rent_per_sec: lease.rent_per_sec,
                        deposit_amount: lease.deposit_amount,
                        security_deposit: lease.security_deposit,
                        start_date: lease.start_date,
                        end_date: lease.end_date,
                        property_uri: lease.property_uri,
                        status: lease.status,
                        payment_token: lease.payment_token,
                        rent_paid: lease.rent_paid,
                        cumulative_payments: lease.cumulative_payments,
                        debt: lease.debt,
                        active: lease.active,
                        yield_delegation_enabled: lease.yield_delegation_enabled,
                        equity_percentage_bps: lease.equity_percentage_bps,
                    };
                    active_leases.push_back(summary);
                }
            }
        }

        active_leases
    }

    pub fn add_to_active_leases_index(env: &Env, lease_id: u64) -> Result<(), LeaseError> {
        let mut index: soroban_sdk::Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::ActiveLeasesIndex)
            .unwrap_or(soroban_sdk::Vec::new(&env));

        index.push_back(lease_id);
        env.storage()
            .instance()
            .set(&DataKey::ActiveLeasesIndex, &index);

        Ok(())
    }

    pub fn remove_from_active_leases_index(env: &Env, lease_id: u64) -> Result<(), LeaseError> {
        let mut index: soroban_sdk::Vec<u64> = env
            .storage()
            .instance()
            .get(&DataKey::ActiveLeasesIndex)
            .unwrap_or(soroban_sdk::Vec::new(&env));

        let mut new_index = soroban_sdk::Vec::new(&env);
        for id in index.iter() {
            if id != lease_id {
                new_index.push_back(id);
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::ActiveLeasesIndex, &new_index);

        Ok(())
    }

    // ========================================================================
    // Issue #132: Ledger Rent Sweeper for Expired Lease Proposals
    // ========================================================================

    /// Permissionlessly sweep a single expired pending lease proposal.
    ///
    /// Any network relayer may call this function to delete a `Pending` lease
    /// that has exceeded its 7-day initialization timeout without receiving a
    /// security deposit. The relayer receives a small gas bounty from the
    /// platform fee vault as an incentive.
    ///
    /// # Parameters
    /// - `relayer`  – Address of the caller; receives the gas bounty.
    /// - `lease_id` – ID of the candidate lease to sweep.
    ///
    /// # Errors
    /// - [`LeaseError::LeaseNotFound`]   – No lease exists for `lease_id`.
    /// - [`LeaseError::InvalidState`]    – Lease is not in `Pending` status.
    /// - [`LeaseError::LeaseNotExpired`] – Initialization timeout has not elapsed.
    pub fn sweep_expired_proposals(
        env: Env,
        relayer: Address,
        lease_id: u64,
    ) -> Result<(), LeaseError> {
        expired_proposals::sweep_expired_proposals(&env, relayer, lease_id)
    }
}

mod test;
mod upgrade_tests;
mod oracle_fallback_tests;
mod governance_tests;

// Global Escrow Freeze Circuit Breaker Modules
pub mod escrow_vault;
pub mod continuous_billing_module;
pub mod escrow_freeze_tests;

// Flash Crash Protection Modules - Issue #114
pub mod collateral_health_monitor;
pub mod collateral_health_tests;

// Issue #132: Ledger Rent Sweeper for Expired Lease Proposals
pub mod expired_proposals;

// Issue #131: Performance Stress Test — 500 Concurrent Lease Actions
#[cfg(test)]
mod stress_tests;
