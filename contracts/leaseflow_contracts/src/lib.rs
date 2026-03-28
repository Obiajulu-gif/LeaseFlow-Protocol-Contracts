#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::enum_variant_names)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address,
    BytesN, Env, String, Symbol,
};

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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UtilityBillStatus {
    Pending,
    Paid,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubletStatus {
    Inactive,
    Active,
    Terminated,
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
pub struct UtilityBill {
    pub lease_id: u64,
    pub bill_hash: BytesN<32>,
    pub usdc_amount: i128,
    pub created_at: u64,
    pub due_date: u64,
    pub status: UtilityBillStatus,
    pub paid_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubletAgreement {
    pub lease_id: u64,
    pub original_tenant: Address,
    pub sub_tenant: Address,
    pub start_date: u64,
    pub end_date: u64,
    pub rent_amount: i128,
    pub landlord_percentage_bps: u32, // Basis points (e.g., 8000 = 80%)
    pub tenant_percentage_bps: u32,    // Basis points (e.g., 2000 = 20%)
    pub status: SubletStatus,
    pub created_at: u64,
    pub total_collected: i128,
    pub landlord_share: i128,
    pub tenant_share: i128,
}

// ── Structs ───────────────────────────────────────────────────────────────────

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
    /// Emergency pause state for natural disasters or force majeure events
    pub paused: bool,
    /// Reason for the emergency pause
    pub pause_reason: Option<String>,
    /// Timestamp when the lease was paused
    pub paused_at: Option<u64>,
    /// Address that initiated the pause (admin, landlord, or trusted third party)
    pub pause_initiator: Option<Address>,
    /// Total time spent in paused state (to adjust rent calculations)
    pub total_paused_duration: u64,
    /// Monthly rent pull authorization - amount approved for automatic withdrawal
    pub rent_pull_authorized_amount: Option<i128>,
    /// Timestamp of the last rent pull execution
    pub last_rent_pull_timestamp: Option<u64>,
    /// Billing cycle duration in seconds (default: 30 days = 2,592,000 seconds)
    pub billing_cycle_duration: u64,
    
    // --- New Features Data ---
    /// [ISSUE 32] Security Deposit Yield Delegation
    pub yield_delegation_enabled: bool,
    pub yield_accumulated: i128,
    /// [ISSUE 33] Rent-to-Own Equity Tracker
    pub equity_balance: i128,
    pub equity_percentage_bps: u32,
    /// [ISSUE 34] Tenant Credit History Tracking
    pub had_late_payment: bool,
    /// [ISSUE 35] Pet Deposit and Monthly Pet Rent
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    /// [ISSUE 36] Utility Pass-Through Billing
    pub next_utility_bill_id: u64,
    pub total_utility_billed: i128,
    pub total_utility_paid: i128,
    /// [ISSUE 37] Subletting Authorization and Fee Split
    pub sublet_enabled: bool,
    pub sub_tenant: Option<Address>,
    pub sublet_start_date: Option<u64>,
    pub sublet_end_date: Option<u64>,
    pub sublet_landlord_percentage_bps: u32,
    pub sublet_tenant_percentage_bps: u32,
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
    pub rent_per_sec: i128,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub late_fee_per_sec: i128,
    pub arbitrators: soroban_sdk::Vec<Address>,
    // New Feature Params
    pub equity_percentage_bps: u32,
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    pub yield_delegation_enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
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
    UtilityBill(u64, u64), // lease_id, bill_id
    SubletAgreement(u64),  // lease_id
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalLease {
    pub lease: LeaseInstance,
    pub terminated_by: Address,
    pub terminated_at: u64,
}

// ── Events ────────────────────────────────────────────────────────────────────

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
pub struct EmergencyRentPaused {
    pub lease_id: u64,
    pub initiator: Address,
    pub reason: String,
    pub paused_at: u64,
}

#[contractevent]
pub struct EmergencyRentResumed {
    pub lease_id: u64,
    pub initiator: Address,
    pub resumed_at: u64,
    pub total_paused_duration: u64,
}

#[contractevent]
pub struct RentPullAuthorized {
    pub lease_id: u64,
    pub tenant: Address,
    pub authorized_amount: i128,
    pub billing_cycle_duration: u64,
}

#[contractevent]
pub struct RentPullExecuted {
    pub lease_id: u64,
    pub landlord: Address,
    pub amount_pulled: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct RentPullRevoked {
    pub lease_id: u64,
    pub tenant: Address,
    pub timestamp: u64,
}

#[contractevent]
pub struct YieldDistributed {
    pub lease_id: u64,
    pub tenant_yield: i128,
    pub landlord_yield: i128,
}

#[contractevent]
pub struct EquityAccrued {
    pub lease_id: u64,
    pub amount: i128,
    pub total_equity: i128,
}

#[contractevent]
pub struct PetStatusChanged {
    pub lease_id: u64,
    pub has_pet: bool,
}

#[contractevent]
pub struct ResidencyNftMinted {
    pub lease_id: u64,
    pub tenant: Address,
}

#[contractevent]
pub struct UtilityBillRequested {
    pub lease_id: u64,
    pub bill_id: u64,
    pub bill_hash: BytesN<32>,
    pub usdc_amount: i128,
    pub due_date: u64,
}

#[contractevent]
pub struct UtilityBillPaid {
    pub lease_id: u64,
    pub bill_id: u64,
    pub tenant: Address,
    pub amount: i128,
    pub paid_at: u64,
}

#[contractevent]
pub struct SubletAuthorized {
    pub lease_id: u64,
    pub original_tenant: Address,
    pub sub_tenant: Address,
    pub start_date: u64,
    pub end_date: u64,
    pub rent_amount: i128,
    pub landlord_percentage_bps: u32,
    pub tenant_percentage_bps: u32,
}

#[contractevent]
pub struct SubletRentPaid {
    pub lease_id: u64,
    pub sub_tenant: Address,
    pub amount: i128,
    pub landlord_share: i128,
    pub tenant_share: i128,
}

#[contractevent]
pub struct SubletTerminated {
    pub lease_id: u64,
    pub terminated_by: Address,
    pub terminated_at: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

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
    WithdrawalAddressNotSet = 12,
    NotAnArbitrator = 13,
    LeaseAlreadyPaused = 14,
    LeaseNotPaused = 15,
    InvalidPauseReason = 16,
    RentPullNotAuthorized = 17,
    BillingCycleNotElapsed = 18,
    InsufficientAuthorizedAmount = 19,
    PaymentTokenMismatch = 20,
    WithdrawalAddressMismatch = 21,
    YieldDelegationNotEnabled = 22,
    PetAlreadyExists = 23,
    PetNotFound = 24,
    IneligibleForResidencyNft = 25,
    InvalidPercentage = 26,
    UtilityBillNotFound = 27,
    UtilityBillAlreadyPaid = 28,
    UtilityBillExpired = 29,
    InvalidAmount = 30,
    SubletAlreadyEnabled = 31,
    SubletNotEnabled = 32,
    SubletAgreementNotFound = 33,
    InvalidSubletDates = 34,
    InvalidPercentageSplit = 35,
    SubletTenantUnauthorized = 36,
    LeaseAlreadyExists = 37,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

pub fn save_utility_bill(env: &Env, lease_id: u64, bill_id: u64, bill: &UtilityBill) {
    let key = DataKey::UtilityBill(lease_id, bill_id);
    env.storage().persistent().set(&key, bill);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_utility_bill(env: &Env, lease_id: u64, bill_id: u64) -> Option<UtilityBill> {
    env.storage()
        .persistent()
        .get(&DataKey::UtilityBill(lease_id, bill_id))
}

pub fn save_sublet_agreement(env: &Env, lease_id: u64, agreement: &SubletAgreement) {
    let key = DataKey::SubletAgreement(lease_id);
    env.storage().persistent().set(&key, agreement);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_sublet_agreement(env: &Env, lease_id: u64) -> Option<SubletAgreement> {
    env.storage()
        .persistent()
        .get(&DataKey::SubletAgreement(lease_id))
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

mod kyc_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "KycClient")]
    pub trait KycInterface {
        fn is_verified(env: Env, address: Address) -> bool;
    }
}

// ── Contract ──────────────────────────────────────────────────────────────────

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

    fn is_asset_allowed(env: &Env, token: &Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::AllowedAsset(token.clone()))
    }

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

    // --- SIMPLE LEASE (Symbol-based) ---

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

    pub fn create_lease_with_nft(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        rent_rate_type: RateType,
        duration: u64,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_amount: i128,
        late_fee_rate_type: RateType,
        nft_contract_addr: Address,
        token_id: u128,
        payment_token: Address,
    ) -> Result<Symbol, LeaseError> {
        // --- ISSUE #29: DOUBLE SIGN PREVENTION ---
        if env.storage().instance().has(&lease_id) {
            return Err(LeaseError::LeaseAlreadyExists);
        }
        // -----------------------------------------

        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &env.current_contract_address(),
            &token_id,
        );

        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);
        let lease = Lease {
            landlord,
            tenant: tenant.clone(),
            rent_per_sec: to_per_second(rent_amount, rent_rate_type),
            late_fee_per_sec: to_per_second(late_fee_amount, late_fee_rate_type),
            deposit_amount: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Active,
            nft_contract: Some(nft_contract_addr.clone()),
            token_id: Some(token_id),
            active: true,
            grace_period_end,
            late_fee_flat,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time,
            buyout_price: None,
            cumulative_payments: 0,
            payment_token,
        };

        save_usage_rights(
            &env,
            nft_contract_addr.clone(),
            token_id,
            &UsageRights {
                renter: tenant,
                nft_contract: lease.nft_contract.clone().unwrap(),
                token_id,
                lease_id: lease_id.clone(),
                valid_until: expiry_time,
            },
        );

        env.storage().instance().set(&lease_id, &lease);
        Ok(symbol_short!("created"))
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
                        token_id,
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

    // --- LEASE INSTANCE (u64-based) ---

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
        let lease = LeaseInstance {
            landlord,
            tenant: params.tenant,
            rent_amount: params.rent_amount,
            deposit_amount: params.deposit_amount,
            security_deposit: params.security_deposit,
            start_date: params.start_date,
            end_date: params.end_date,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            status: LeaseStatus::Pending,
            property_uri: params.property_uri,
            nft_contract: None,
            token_id: None,
            active: true,
            debt: 0,
            rent_paid: 0,
            expiry_time: params.end_date,
            buyout_price: None,
            cumulative_payments: 0,
            rent_per_sec: 0,
            grace_period_end: params.end_date,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            billing_cycle_duration: 2_592_000, // 30 days in seconds
            // New Features Initialization
            yield_delegation_enabled: params.yield_delegation_enabled,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: params.equity_percentage_bps,
            had_late_payment: false,
            has_pet: params.has_pet,
            pet_deposit_amount: params.pet_deposit_amount,
            pet_rent_amount: params.pet_rent_amount,
            // Utility Billing Initialization
            next_utility_bill_id: 1,
            total_utility_billed: 0,
            total_utility_paid: 0,
            // Subletting Initialization
            sublet_enabled: false,
            sub_tenant: None,
            sublet_start_date: None,
            sublet_end_date: None,
            sublet_landlord_percentage_bps: 8000, // Default 80% to landlord
            sublet_tenant_percentage_bps: 2000,   // Default 20% to original tenant
        };
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn get_lease_instance(env: Env, lease_id: u64) -> Result<LeaseInstance, LeaseError> {
        load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound);
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

        let balance_key = DataKey::RoommateBalance(lease_id, payer.clone());
        let mut payer_bal: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        payer_bal += payment_amount;
        env.storage().persistent().set(&balance_key, &payer_bal);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);

        // Update Cumulative Payments
        lease.cumulative_payments += payment_amount;

        // [ISSUE 33] Calculate Equity Portion
        let equity_amount = (payment_amount * (lease.equity_percentage_bps as i128)) / 10000;
        lease.equity_balance += equity_amount;
        
        // [ISSUE 34] Late Payment Tracking
        // Check if the tenant is currently in debt before this payment
        if lease.debt > 0 {
            lease.had_late_payment = true;
        }

        // Rent portion available for landlord withdrawal (excludes equity)
        let rent_to_lanlord = payment_amount - equity_amount;
        lease.rent_paid += rent_to_lanlord;

        // token_client.transfer(&payer, &env.current_contract_address(), &payment_amount);

        if equity_amount > 0 {
            EquityAccrued {
                lease_id,
                amount: equity_amount,
                total_equity: lease.equity_balance,
            }.publish(&env);
        }

        RentPaidPartial { lease_id, roommate: payer.clone(), amount: payment_amount }.publish(&env);

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

        // token_client.transfer(&env.current_contract_address(), &_withdrawal_address, &_withdrawable_amount);

        lease.rent_withdrawn += _withdrawable_amount;
        save_lease_instance(&env, lease_id, &lease);

        Ok(())
    }

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

        archive_lease(&env, lease_id, lease, caller);
        LeaseTerminated { lease_id }.publish(&env);
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
    pub fn conclude_lease(env: Env, lease_id: u64, landlord: Address, damage_deduction: i128) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if landlord != lease.landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();
        Self::require_kyc(&env, &lease.landlord, &lease.tenant)?;

        if env.ledger().timestamp() < lease.end_date { return Err(LeaseError::LeaseNotExpired); }

        if damage_deduction < 0 || damage_deduction > lease.deposit_amount {
            return Err(LeaseError::InvalidDeduction);
        }

        // [ISSUE 34] Create Tenant Credit History NFT Minter
        // Criteria: 12-month lease completion, no late payments
        let duration = lease.end_date.saturating_sub(lease.start_date);
        let year_in_seconds = 31_536_000;
        if duration >= year_in_seconds && !lease.had_late_payment {
            ResidencyNftMinted {
                lease_id,
                tenant: lease.tenant.clone(),
            }.publish(&env);
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
        landlord.require_auth();
        
        // 3. Validate damage deduction
        if damage_deduction < 0 || damage_deduction > lease.security_deposit {
            return Err(LeaseError::InvalidDeduction);
        }

        let refund_amount = lease.security_deposit - damage_deduction;

        // [ISSUE 34] Create Tenant Credit History NFT Minter
        // Criteria: 12-month lease completion, no late payments
        let duration = lease.end_date.saturating_sub(lease.start_date);
        let year_in_seconds = 31_536_000;
        if duration >= year_in_seconds && !lease.had_late_payment {
            ResidencyNftMinted {
                lease_id,
                tenant: lease.tenant.clone(),
            }.publish(&env);
            // In a real implementation, we would call an NFT contract here to mint.
            // For now, publishing the event serves as the "Digital Resume" trigger.
        }

        // 4. Update lease state
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;
        save_lease_instance(&env, lease_id, &lease);

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

    pub fn check_tenant_default(env: Env, lease_id: u64) -> Result<i128, LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        let current_time = env.ledger().timestamp();
        
        // Calculate effective elapsed time (excluding paused periods)
        let mut effective_elapsed_secs = current_time.saturating_sub(lease.start_date);
        
        // Subtract total paused duration from elapsed time
        effective_elapsed_secs = effective_elapsed_secs.saturating_sub(lease.total_paused_duration);
        
        // If currently paused, subtract current pause duration
        if lease.paused {
            if let Some(paused_at) = lease.paused_at {
                let current_pause_duration = current_time.saturating_sub(paused_at);
                effective_elapsed_secs = effective_elapsed_secs.saturating_sub(current_pause_duration);
            }
        }

        // [ISSUE 35] Include Pet Rent in expected calculations
        // We calculate pet rent based on elapsed billing cycles
        let billing_cycles = effective_elapsed_secs / lease.billing_cycle_duration;
        let accrued_pet_rent = (billing_cycles as i128) * lease.pet_rent_amount;
        
        let expected_rent = (effective_elapsed_secs as i128).saturating_mul(lease.rent_per_sec) + accrued_pet_rent;
        let unpaid_rent = expected_rent.saturating_sub(lease.rent_paid);
        let mut total_debt = if unpaid_rent > 0 { unpaid_rent } else { 0 };

        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;

            if seconds_late > 0 {
                // [ISSUE 34] Mark as had late payment
                lease.had_late_payment = true;
            }

            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            if seconds_late > lease.seconds_late_charged {
                let newly_accrued = seconds_late - lease.seconds_late_charged;
                lease.debt += (newly_accrued as i128) * lease.late_fee_per_sec;
                lease.seconds_late_charged = seconds_late;
            }
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

    // --- [ISSUE 32] Security Deposit Yield Delegation ---

    pub fn enable_yield_delegation(env: Env, lease_id: u64, tenant: Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.tenant != tenant { return Err(LeaseError::Unauthorised); }
        tenant.require_auth();

        lease.yield_delegation_enabled = true;
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    /// Hook for a "Safe" liquidity pool to distribute earned yield.
    /// Split interest between landlord and tenant.
    pub fn distribute_yield(env: Env, lease_id: u64, yield_amount: i128) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if !lease.yield_delegation_enabled { return Err(LeaseError::YieldDelegationNotEnabled); }

        // Split yield 50/50 between landlord and tenant
        let half_yield = yield_amount / 2;
        
        // Tenant's portion increases their security deposit (productive asset)
        // Landlord's portion is added to their withdrawable rent balance
        lease.security_deposit += half_yield;
        lease.rent_paid += half_yield;
        lease.yield_accumulated += yield_amount;

        save_lease_instance(&env, lease_id, &lease);

        YieldDistributed {
            lease_id,
            tenant_yield: half_yield,
            landlord_yield: half_yield,
        }.publish(&env);

        Ok(())
    }

    // --- [ISSUE 35] Pet Management ---

    pub fn toggle_pet(env: Env, lease_id: u64, landlord: Address, has_pet: bool, pet_deposit: i128, pet_rent: i128) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();

        lease.has_pet = has_pet;
        lease.pet_deposit_amount = pet_deposit;
        lease.pet_rent_amount = pet_rent;
        
        // If pet is added, we might need a pet deposit payment from tenant.
        // For simplicity, we assume the deposit is handled via a separate payment or upfront.

        save_lease_instance(&env, lease_id, &lease);
        
        PetStatusChanged { lease_id, has_pet }.publish(&env);
        Ok(())
    }

    /// Handles partial refund of the pet deposit while keeping main security deposit intact.
    pub fn refund_pet_deposit(env: Env, lease_id: u64, landlord: Address, amount: i128) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();

        if amount > lease.pet_deposit_amount { return Err(LeaseError::InvalidDeduction); }

        let refund_amount = amount;
        lease.pet_deposit_amount -= amount;

        save_lease_instance(&env, lease_id, &lease);
        Ok(refund_amount)
    }

    /// Authorizes an additional roommate to make payments towards a lease.
    pub fn add_authorized_payer(env: Env, lease_id: u64, landlord: Address, roommate: Address) -> Result<(), LeaseError> {
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

    // --- [ISSUE 36] Utility Pass-Through Billing ---

    /// Landlord requests utility payment from tenant by uploading bill hash and USDC amount
    /// Tenant has 7 days to pay the utility bill through the contract
    pub fn request_utility_payment(
        env: Env,
        lease_id: u64,
        landlord: Address,
        bill_hash: BytesN<32>,
        usdc_amount: i128,
    ) -> Result<u64, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        
        if usdc_amount <= 0 {
            return Err(LeaseError::InvalidAmount);
        }
        
        landlord.require_auth();
        
        let now = env.ledger().timestamp();
        let due_date = now + (7 * 24 * 60 * 60); // 7 days from now
        
        let bill_id = lease.next_utility_bill_id;
        let utility_bill = UtilityBill {
            lease_id,
            bill_hash: bill_hash.clone(),
            usdc_amount,
            created_at: now,
            due_date,
            status: UtilityBillStatus::Pending,
            paid_at: None,
        };
        
        // Save the utility bill
        save_utility_bill(&env, lease_id, bill_id, &utility_bill);
        
        // Update lease state
        lease.next_utility_bill_id += 1;
        lease.total_utility_billed += usdc_amount;
        save_lease_instance(&env, lease_id, &lease);
        
        // Publish event
        UtilityBillRequested {
            lease_id,
            bill_id,
            bill_hash,
            usdc_amount,
            due_date,
        }.publish(&env);
        
        Ok(bill_id)
    }
    
    /// Tenant pays a utility bill
    pub fn pay_utility_bill(
        env: Env,
        lease_id: u64,
        bill_id: u64,
        tenant: Address,
        payment_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if lease.tenant != tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        let mut utility_bill = load_utility_bill(&env, lease_id, bill_id)
            .ok_or(LeaseError::UtilityBillNotFound)?;
            
        if utility_bill.status != UtilityBillStatus::Pending {
            return Err(LeaseError::UtilityBillAlreadyPaid);
        }
        
        let now = env.ledger().timestamp();
        
        // Check if bill has expired (7 days past due date)
        if now > utility_bill.due_date {
            utility_bill.status = UtilityBillStatus::Expired;
            save_utility_bill(&env, lease_id, bill_id, &utility_bill);
            return Err(LeaseError::UtilityBillExpired);
        }
        
        if payment_amount != utility_bill.usdc_amount {
            return Err(LeaseError::InvalidAmount);
        }
        
        tenant.require_auth();
        
        // Update utility bill status
        utility_bill.status = UtilityBillStatus::Paid;
        utility_bill.paid_at = Some(now);
        save_utility_bill(&env, lease_id, bill_id, &utility_bill);
        
        // Update lease totals
        lease.total_utility_paid += payment_amount;
        save_lease_instance(&env, lease_id, &lease);
        
        // Publish event
        UtilityBillPaid {
            lease_id,
            bill_id,
            tenant,
            amount: payment_amount,
            paid_at: now,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get details of a specific utility bill
    pub fn get_utility_bill(env: Env, lease_id: u64, bill_id: u64) -> Result<UtilityBill, LeaseError> {
        load_utility_bill(&env, lease_id, bill_id).ok_or(LeaseError::UtilityBillNotFound)
    }
    
    /// Get all utility bills for a lease (returns count, actual bills would need pagination)
    pub fn get_utility_bill_count(env: Env, lease_id: u64) -> Result<u64, LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        Ok(lease.next_utility_bill_id - 1)
    }

    // --- [ISSUE 37] Subletting Authorization and Fee Split ---

    /// Original tenant authorizes a sub-tenant with specified rent and percentage split
    pub fn authorize_sublet(
        env: Env,
        lease_id: u64,
        original_tenant: Address,
        sub_tenant: Address,
        start_date: u64,
        end_date: u64,
        rent_amount: i128,
        landlord_percentage_bps: u32,
        tenant_percentage_bps: u32,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if lease.tenant != original_tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        if lease.sublet_enabled {
            return Err(LeaseError::SubletAlreadyEnabled);
        }
        
        // Validate percentage split (must add up to 10000 = 100%)
        if landlord_percentage_bps + tenant_percentage_bps != 10000 {
            return Err(LeaseError::InvalidPercentageSplit);
        }
        
        // Validate dates
        let now = env.ledger().timestamp();
        if start_date < now || end_date <= start_date || end_date > lease.end_date {
            return Err(LeaseError::InvalidSubletDates);
        }
        
        if rent_amount <= 0 {
            return Err(LeaseError::InvalidAmount);
        }
        
        original_tenant.require_auth();
        
        // Create sublet agreement
        let sublet_agreement = SubletAgreement {
            lease_id,
            original_tenant: original_tenant.clone(),
            sub_tenant: sub_tenant.clone(),
            start_date,
            end_date,
            rent_amount,
            landlord_percentage_bps,
            tenant_percentage_bps,
            status: SubletStatus::Active,
            created_at: now,
            total_collected: 0,
            landlord_share: 0,
            tenant_share: 0,
        };
        
        // Update lease state
        lease.sublet_enabled = true;
        lease.sub_tenant = Some(sub_tenant.clone());
        lease.sublet_start_date = Some(start_date);
        lease.sublet_end_date = Some(end_date);
        lease.sublet_landlord_percentage_bps = landlord_percentage_bps;
        lease.sublet_tenant_percentage_bps = tenant_percentage_bps;
        
        // Save changes
        save_sublet_agreement(&env, lease_id, &sublet_agreement);
        save_lease_instance(&env, lease_id, &lease);
        
        // Publish event
        SubletAuthorized {
            lease_id,
            original_tenant,
            sub_tenant,
            start_date,
            end_date,
            rent_amount,
            landlord_percentage_bps,
            tenant_percentage_bps,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Sub-tenant pays rent, which gets split between landlord and original tenant
    pub fn pay_sublet_rent(
        env: Env,
        lease_id: u64,
        sub_tenant: Address,
        payment_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if !lease.sublet_enabled {
            return Err(LeaseError::SubletNotEnabled);
        }
        
        if lease.sub_tenant.as_ref() != Some(&sub_tenant) {
            return Err(LeaseError::SubletTenantUnauthorized);
        }
        
        let mut sublet_agreement = load_sublet_agreement(&env, lease_id)
            .ok_or(LeaseError::SubletAgreementNotFound)?;
            
        if sublet_agreement.status != SubletStatus::Active {
            return Err(LeaseError::SubletNotEnabled);
        }
        
        let now = env.ledger().timestamp();
        
        // Check if sublet is still valid
        if now < sublet_agreement.start_date || now > sublet_agreement.end_date {
            return Err(LeaseError::InvalidSubletDates);
        }
        
        if payment_amount != sublet_agreement.rent_amount {
            return Err(LeaseError::InvalidAmount);
        }
        
        sub_tenant.require_auth();
        
        // Calculate splits
        let landlord_share = (payment_amount * (sublet_agreement.landlord_percentage_bps as i128)) / 10000;
        let tenant_share = payment_amount - landlord_share;
        
        // Update sublet agreement
        sublet_agreement.total_collected += payment_amount;
        sublet_agreement.landlord_share += landlord_share;
        sublet_agreement.tenant_share += tenant_share;
        save_sublet_agreement(&env, lease_id, &sublet_agreement);
        
        // Update lease rent tracking (landlord portion counts as rent paid)
        lease.rent_paid += landlord_share;
        lease.cumulative_payments += payment_amount;
        save_lease_instance(&env, lease_id, &lease);
        
        // Publish event
        SubletRentPaid {
            lease_id,
            sub_tenant,
            amount: payment_amount,
            landlord_share,
            tenant_share,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Terminate sublet agreement (can be called by original tenant or landlord)
    pub fn terminate_sublet(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if !lease.sublet_enabled {
            return Err(LeaseError::SubletNotEnabled);
        }
        
        let is_original_tenant = caller == lease.tenant;
        let is_landlord = caller == lease.landlord;
        
        if !is_original_tenant && !is_landlord {
            return Err(LeaseError::Unauthorised);
        }
        
        caller.require_auth();
        
        let mut sublet_agreement = load_sublet_agreement(&env, lease_id)
            .ok_or(LeaseError::SubletAgreementNotFound)?;
            
        sublet_agreement.status = SubletStatus::Terminated;
        save_sublet_agreement(&env, lease_id, &sublet_agreement);
        
        // Reset lease subletting state
        lease.sublet_enabled = false;
        lease.sub_tenant = None;
        lease.sublet_start_date = None;
        lease.sublet_end_date = None;
        save_lease_instance(&env, lease_id, &lease);
        
        // Publish event
        SubletTerminated {
            lease_id,
            terminated_by: caller,
            terminated_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get sublet agreement details
    pub fn get_sublet_agreement(env: Env, lease_id: u64) -> Result<SubletAgreement, LeaseError> {
        load_sublet_agreement(&env, lease_id).ok_or(LeaseError::SubletAgreementNotFound)
    }
}

mod test;
