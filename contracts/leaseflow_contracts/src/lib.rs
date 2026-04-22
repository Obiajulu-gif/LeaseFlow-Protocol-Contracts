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
    InArbitration,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
    InArbitration,
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
    pub wear_allowance_bps: u32,
    pub asset_lifespan_days: u64,
    pub asset_value: i128,
    pub deposit_timestamp: u64,
    pub subleasing_allowed: bool,
    pub master_lease_id: Option<u64>,
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
    pub wear_allowance_bps: u32,
    pub asset_lifespan_days: u64,
    pub asset_value: i128,
    pub subleasing_allowed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeCase {
    pub lease_id: u64,
    pub challenger: Address,
    pub dispute_timestamp: u64,
    pub dispute_bond: i128,
    pub selected_jurors: soroban_sdk::Vec<Address>,
    pub juror_votes: soroban_sdk::Vec<bool>,
    pub verdict_deadline: u64,
    pub is_resolved: bool,
    pub resolution: Option<DepositReleasePartial>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Juror {
    pub address: Address,
    pub reputation: u32,
    pub stake_amount: i128,
    pub cases_participated: u32,
    pub successful_votes: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubEscrowVault {
    pub master_lease_id: u64,
    pub sub_lease_id: u64,
    pub sub_lessee: Address,
    pub deposit_amount: i128,
    pub is_active: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSubleaseParams {
    pub master_lease_id: u64,
    pub sub_lessee: Address,
    pub sub_rent_amount: i128,
    pub sub_deposit_amount: i128,
    pub sub_start_date: u64,
    pub sub_end_date: u64,
    pub property_uri: String,
    pub payment_token: Address,
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
    PlatformFeeAmount,
    PlatformFeeToken,
    PlatformFeeRecipient,
    TermsHash,
    DisputeCase(u64),
    Juror(Address),
    JurorPool,
    SubEscrowVault(u64),
    SubLeaseCounter,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalLease {
    pub lease: LeaseInstance,
    pub terminated_by: Address,
    pub terminated_at: u64,
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
pub struct PaymentLate {
    pub lease_id: u64,
    pub days_late: u64,
    pub current_fine: i128,
}

#[contractevent]
pub struct LeaseStarted {
    pub id: u64,
    pub renter: Address,
    pub rate: i128,
}

#[contractevent]
pub struct LeaseSigned {
    pub lease_id: u64,
    pub property_hash: String,
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
pub struct WearAndTearCalculated {
    pub lease_id: u64,
    pub allowed_decay: i128,
    pub reported_decay: i128,
    pub elapsed_days: u64,
    pub wear_allowance_bps: u32,
}

#[contractevent]
pub struct SettlementPeriodStarted {
    pub lease_id: u64,
    pub deposit_timestamp: u64,
    pub settlement_ledgers: u32,
}

#[contractevent]
pub struct EvictionEligible {
    pub lease_id: u64,
    pub tenant: Address,
    pub debt: i128,
}

#[contractevent]
pub struct DisputeRaised {
    pub lease_id: u64,
    pub challenger: Address,
    pub dispute_bond: i128,
    pub selected_jurors: soroban_sdk::Vec<Address>,
    pub verdict_deadline: u64,
}

#[contractevent]
pub struct JurorSelected {
    pub lease_id: u64,
    pub juror: Address,
}

#[contractevent]
pub struct JurorVerdict {
    pub lease_id: u64,
    pub juror: Address,
    pub vote: bool,
}

#[contractevent]
pub struct JurorSlashed {
    pub juror: Address,
    pub slash_amount: i128,
    pub reason: String,
}

#[contractevent]
pub struct SubleaseCreated {
    pub master_lease_id: u64,
    pub sub_lease_id: u64,
    pub sub_lessee: Address,
    pub sub_escrow_vault_id: u64,
}

#[contractevent]
pub struct SubleaseTerminated {
    pub master_lease_id: u64,
    pub sub_lease_id: u64,
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
    InvalidProrationMath = 17,
    FlashLoanAttemptBlocked = 18,
    SettlementPeriodNotElapsed = 19,
    DisputeWindowExpired = 20,
    InsufficientDisputeBond = 21,
    DisputeAlreadyActive = 22,
    JurorNotFound = 23,
    InsufficientJurorStake = 24,
    JurorSelectionFailed = 25,
    VerdictDeadlinePassed = 26,
    InsufficientJurorVotes = 27,
    SubleasingNotAllowed = 28,
    SubleaseBoundaryExceeded = 29,
    MasterLeaseNotFound = 30,
    SubEscrowVaultNotFound = 31,
    JurorReputationTooLow = 32,
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

// Dispute resolution constants
const DISPUTE_WINDOW_HOURS: u64 = 48;
const DISPUTE_WINDOW_LEDGERS: u64 = DISPUTE_WINDOW_HOURS * 720; // 720 ledgers per hour
const JURY_SIZE: u32 = 3;
const JURY_VOTE_THRESHOLD: u32 = 2; // 2-of-3 multi-sig
const JUROR_VOTE_DEADLINE_HOURS: u64 = 72;
const JUROR_VOTE_DEADLINE_LEDGERS: u64 = JUROR_VOTE_DEADLINE_HOURS * 720;
const MIN_JUROR_REPUTATION: u32 = 100;
const MIN_JUROR_STAKE: i128 = 1_000_000; // 0.001 XLM equivalent
const DISPUTE_BOND_AMOUNT: i128 = 5_000_000; // 0.005 XLM equivalent
const JUROR_SLASH_AMOUNT: i128 = 2_000_000; // 0.002 XLM equivalent

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

// Dispute resolution helper functions
pub fn save_dispute_case(env: &Env, dispute_id: u64, dispute_case: &DisputeCase) {
    let key = DataKey::DisputeCase(dispute_id);
    env.storage().persistent().set(&key, dispute_case);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_dispute_case(env: &Env, dispute_id: u64) -> Option<DisputeCase> {
    env.storage()
        .persistent()
        .get(&DataKey::DisputeCase(dispute_id))
}

pub fn save_juror(env: &Env, juror_address: &Address, juror: &Juror) {
    let key = DataKey::Juror(juror_address.clone());
    env.storage().persistent().set(&key, juror);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_juror(env: &Env, juror_address: &Address) -> Option<Juror> {
    env.storage()
        .persistent()
        .get(&DataKey::Juror(juror_address.clone()))
}

pub fn get_juror_pool(env: &Env) -> soroban_sdk::Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::JurorPool)
        .unwrap_or(soroban_sdk::Vec::new(env))
}

pub fn save_juror_pool(env: &Env, pool: &soroban_sdk::Vec<Address>) {
    env.storage().persistent().set(&DataKey::JurorPool, pool);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::JurorPool, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn save_sub_escrow_vault(env: &Env, vault_id: u64, vault: &SubEscrowVault) {
    let key = DataKey::SubEscrowVault(vault_id);
    env.storage().persistent().set(&key, vault);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_sub_escrow_vault(env: &Env, vault_id: u64) -> Option<SubEscrowVault> {
    env.storage()
        .persistent()
        .get(&DataKey::SubEscrowVault(vault_id))
}

pub fn get_next_sub_lease_id(env: &Env) -> u64 {
    let counter: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::SubLeaseCounter)
        .unwrap_or(0);
    let next_id = counter + 1;
    env.storage().persistent().set(&DataKey::SubLeaseCounter, &next_id);
    env.storage()
        .persistent()
        .extend_ttl(&DataKey::SubLeaseCounter, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
    next_id
}

// Cryptographically secure random juror selection
pub fn select_random_jurors(env: &Env, pool: &soroban_sdk::Vec<Address>, count: u32) -> soroban_sdk::Vec<Address> {
    let mut selected = soroban_sdk::Vec::new(env);
    let mut available_indices = soroban_sdk::Vec::new(env);
    
    // Create index pool
    for i in 0..pool.len() {
        available_indices.push_back(i);
    }
    
    // Use ledger timestamp and sequence for entropy
    let seed = env.ledger().timestamp() ^ env.ledger().sequence() as u64;
    
    for _ in 0..count {
        if available_indices.is_empty() {
            break;
        }
        
        let random_index = (seed % available_indices.len() as u64) as u32;
        let juror_index = available_indices.get(random_index as u32).unwrap();
        selected.push_back(pool.get(juror_index).unwrap());
        
        // Remove selected index
        available_indices.remove(random_index as u32);
    }
    
    selected
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
        if env.storage().instance().has(&lease_id) {
            return Err(LeaseError::LeaseAlreadyExists);
        }

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
            property_uri: params.property_uri.clone(),
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
            inspector: None,
            wear_allowance_bps: params.wear_allowance_bps,
            asset_lifespan_days: params.asset_lifespan_days,
            asset_value: params.asset_value,
            deposit_timestamp: env.ledger().timestamp(),
            subleasing_allowed: params.subleasing_allowed,
            master_lease_id: None,
        };
        save_lease_instance(&env, lease_id, &lease);
        LeaseSigned {
            lease_id,
            property_hash: params.property_uri.clone(),
        }
        .publish(&env);
        Ok(())
    }

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

        // [ISSUE 5] Pay a 10 % bounty of the platform fee to the caller to
        // incentivise timely lease closure and prevent zombie storage.
        const BOUNTY_BPS: i128 = 1_000; // 10 %
        if let (Some(fee_amount), Some(fee_token), Some(fee_recipient)) = (
            env.storage().instance().get::<DataKey, i128>(&DataKey::PlatformFeeAmount),
            env.storage().instance().get::<DataKey, Address>(&DataKey::PlatformFeeToken),
            env.storage().instance().get::<DataKey, Address>(&DataKey::PlatformFeeRecipient),
        ) {
            let bounty = fee_amount * BOUNTY_BPS / 10_000;
            if bounty > 0 {
                let token = token_contract::TokenClient::new(&env, &fee_token);
                token.transfer(&fee_recipient, &caller, &bounty);
                TerminateBountyPaid { lease_id, caller: caller.clone(), amount: bounty }.publish(&env);
            }
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

    /// [ISSUE 5] Configure the platform fee used to fund terminate bounties.
    /// Only callable by the admin. `fee_amount` is the total platform fee in
    /// token stroops; 10 % of it is paid as a bounty to whoever calls
    /// `terminate_lease`.
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
        env.storage().instance().set(&DataKey::PlatformFeeAmount, &fee_amount);
        env.storage().instance().set(&DataKey::PlatformFeeToken, &fee_token);
        env.storage().instance().set(&DataKey::PlatformFeeRecipient, &fee_recipient);
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

    /// Calculate wear and tear proration for long-term leases
    /// Uses i128 fixed-point math for precision without truncation
    pub fn calculate_wear_proration(
        env: Env,
        lease_id: u64,
        oracle_reported_decay: i128,
    ) -> Result<i128, LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        // Prevent division by zero
        if lease.asset_lifespan_days == 0 {
            return Err(LeaseError::InvalidProrationMath);
        }
        
        let current_time = env.ledger().timestamp();
        let elapsed_seconds = current_time.saturating_sub(lease.start_date);
        let elapsed_days = elapsed_seconds / 86_400; // Convert to days
        
        // Edge case: extremely early termination to prevent abuse
        if elapsed_days < 1 {
            return Ok(0); // No allowance for less than 1 day
        }
        
        // Calculate expected degradation: (elapsed_lease_time / total_expected_lifespan) * asset_value
        // Using i128 fixed-point math: multiply first, then divide to maintain precision
        let expected_degradation = (elapsed_days as i128)
            .saturating_mul(lease.asset_value)
            .saturating_div(lease.asset_lifespan_days as i128);
        
        // Apply wear allowance: expected_degradation * wear_allowance_bps / 10000
        let allowed_decay = expected_degradation
            .saturating_mul(lease.wear_allowance_bps as i128)
            .saturating_div(10_000_i128);
        
        // Round in favor of protocol (ceiling division)
        let protocol_favor_decay = if expected_degradation.saturating_mul(lease.wear_allowance_bps as i128) % 10_000_i128 != 0 {
            allowed_decay + 1
        } else {
            allowed_decay
        };
        
        // Emit event with calculation details
        WearAndTearCalculated {
            lease_id,
            allowed_decay: protocol_favor_decay,
            reported_decay: oracle_reported_decay,
            elapsed_days,
            wear_allowance_bps: lease.wear_allowance_bps,
        }
        .publish(&env);
        
        // If Oracle reported damage falls under allowance, no penalty
        if oracle_reported_decay <= protocol_favor_decay {
            Ok(0) // No deduction
        } else {
            // Return the amount exceeding the allowance
            Ok(oracle_reported_decay - protocol_favor_decay)
        }
    }

    /// Deposit security collateral with flash loan protection
    pub fn deposit_security_collateral(
        env: Env,
        lease_id: u64,
        payer: Address,
        amount: i128,
    ) -> Result<(), LeaseError> {
        payer.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        // Check if this is a potential flash loan attempt
        let current_ledger = env.ledger().sequence() as u64;
        let deposit_ledger = lease.deposit_timestamp;
        
        // Settlement period requirement: 3 ledgers
        const SETTLEMENT_LEDGERS: u32 = 3;
        
        // Check if deposit was made in current or recent ledgers (potential flash loan)
        if current_ledger.saturating_sub(deposit_ledger) < SETTLEMENT_LEDGERS as u64 {
            // Log the attempt and block
            // In a real implementation, you might want to store this in a blacklist
            return Err(LeaseError::FlashLoanAttemptBlocked);
        }
        
        // Update lease status to Active after settlement period
        if lease.status == LeaseStatus::Pending {
            lease.status = LeaseStatus::Active;
            
            SettlementPeriodStarted {
                lease_id,
                deposit_timestamp: lease.deposit_timestamp,
                settlement_ledgers: SETTLEMENT_LEDGERS,
            }
            .publish(&env);
        }
        
        // Handle mid-lease top-ups
        let balance_key = DataKey::RoommateBalance(lease_id, payer.clone());
        let mut current_balance: i128 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        current_balance += amount;
        env.storage().persistent().set(&balance_key, &current_balance);
        env.storage()
            .persistent()
            .extend_ttl(&balance_key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    /// Enhanced conclude_lease with wear and tear integration
    pub fn conclude_lease_wear_proration(
        env: Env,
        lease_id: u64,
        landlord: Address,
        oracle_reported_decay: i128,
    ) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if landlord != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();
        
        // Calculate wear and tear proration
        let wear_deduction = Self::calculate_wear_proration(env.clone(), lease_id, oracle_reported_decay)?;
        
        // Ensure deduction doesn't exceed deposit
        let total_deduction = if wear_deduction > lease.security_deposit {
            lease.security_deposit
        } else {
            wear_deduction
        };
        
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        save_lease_instance(&env, lease_id, &lease);
        
        Ok(lease.security_deposit - total_deduction)
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
        TermsHashUpdated { new_terms_hash: hash }.publish(&env);
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

    // DAO Arbitration Functions
    
    /// Register a juror in the DAO arbitration system
    pub fn register_juror(
        env: Env,
        juror_address: Address,
        stake_amount: i128,
    ) -> Result<(), LeaseError> {
        juror_address.require_auth();
        
        if stake_amount < MIN_JUROR_STAKE {
            return Err(LeaseError::InsufficientJurorStake);
        }
        
        let juror = Juror {
            address: juror_address.clone(),
            reputation: 100, // Starting reputation
            stake_amount,
            cases_participated: 0,
            successful_votes: 0,
        };
        
        save_juror(&env, &juror_address, &juror);
        
        // Add to juror pool
        let mut pool = get_juror_pool(&env);
        if !pool.contains(&juror_address) {
            pool.push_back(juror_address);
            save_juror_pool(&env, &pool);
        }
        
        Ok(())
    }
    
    /// Raise a lease dispute within 48 hours of termination
    pub fn raise_lease_dispute(
        env: Env,
        lease_id: u64,
        challenger: Address,
        dispute_bond: i128,
    ) -> Result<(), LeaseError> {
        challenger.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Verify challenger is either landlord or tenant
        if challenger != lease.landlord && challenger != lease.tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        // Check if dispute window is still open (48 hours from termination)
        let current_ledger = env.ledger().sequence() as u64;
        let termination_ledger = if lease.status == LeaseStatus::Terminated {
            // Use a stored termination timestamp or calculate from end_date
            lease.end_date / 5 // Approximate ledger sequence
        } else {
            return Err(LeaseError::DisputeWindowExpired);
        };
        
        if current_ledger > termination_ledger + DISPUTE_WINDOW_LEDGERS {
            return Err(LeaseError::DisputeWindowExpired);
        }
        
        // Check minimum bond requirement
        if dispute_bond < DISPUTE_BOND_AMOUNT {
            return Err(LeaseError::InsufficientDisputeBond);
        }
        
        // Check if dispute already exists
        if lease.deposit_status == DepositStatus::InArbitration {
            return Err(LeaseError::DisputeAlreadyActive);
        }
        
        // Select 3 random jurors from the pool
        let juror_pool = get_juror_pool(&env);
        if juror_pool.len() < JURY_SIZE as u32 {
            return Err(LeaseError::JurorSelectionFailed);
        }
        
        let selected_jurors = select_random_jurors(&env, &juror_pool, JURY_SIZE);
        
        // Create dispute case
        let dispute_case = DisputeCase {
            lease_id,
            challenger: challenger.clone(),
            dispute_timestamp: env.ledger().timestamp(),
            dispute_bond,
            selected_jurors: selected_jurors.clone(),
            juror_votes: soroban_sdk::Vec::new(&env),
            verdict_deadline: env.ledger().timestamp() + JUROR_VOTE_DEADLINE_LEDGERS,
            is_resolved: false,
            resolution: None,
        };
        
        // Update lease status
        lease.status = LeaseStatus::InArbitration;
        lease.deposit_status = DepositStatus::InArbitration;
        save_lease_instance(&env, lease_id, &lease);
        
        // Save dispute case
        save_dispute_case(&env, lease_id, &dispute_case);
        
        // Emit events
        DisputeRaised {
            lease_id,
            challenger: challenger.clone(),
            dispute_bond,
            selected_jurors: selected_jurors.clone(),
            verdict_deadline: dispute_case.verdict_deadline,
        }.publish(&env);
        
        for juror in selected_jurors.iter() {
            JurorSelected {
                lease_id,
                juror,
            }.publish(&env);
        }
        
        Ok(())
    }
    
    /// Submit a juror's verdict on a dispute case
    pub fn submit_juror_verdict(
        env: Env,
        lease_id: u64,
        juror: Address,
        vote: bool, // true for tenant, false for landlord
        signed_verdict: BytesN<32>,
    ) -> Result<(), LeaseError> {
        juror.require_auth();
        
        let mut dispute_case = load_dispute_case(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Verify juror is selected for this case
        if !dispute_case.selected_jurors.contains(&juror) {
            return Err(LeaseError::NotAnArbitrator);
        }
        
        // Check if verdict deadline has passed
        if env.ledger().timestamp() > dispute_case.verdict_deadline {
            // Slash juror for not voting on time
            let mut juror_data = load_juror(&env, &juror)
                .ok_or(LeaseError::JurorNotFound)?;
            juror_data.stake_amount -= JUROR_SLASH_AMOUNT;
            save_juror(&env, &juror, &juror_data);
            
            JurorSlashed {
                juror: juror.clone(),
                slash_amount: JUROR_SLASH_AMOUNT,
                reason: String::from_str(&env, "Missed verdict deadline"),
            }.publish(&env);
            
            return Err(LeaseError::VerdictDeadlinePassed);
        }
        
        // Check if juror has already voted
        for (i, selected_juror) in dispute_case.selected_jurors.iter().enumerate() {
            if selected_juror == juror {
                if dispute_case.juror_votes.len() > i as u32 {
                    return Err(LeaseError::Unauthorised); // Already voted
                }
                break;
            }
        }
        
        // Add juror vote
        dispute_case.juror_votes.push_back(vote);
        
        // Update juror statistics
        let mut juror_data = load_juror(&env, &juror)
            .ok_or(LeaseError::JurorNotFound)?;
        juror_data.cases_participated += 1;
        save_juror(&env, &juror, &juror_data);
        
        // Emit verdict event
        JurorVerdict {
            lease_id,
            juror: juror.clone(),
            vote,
        }.publish(&env);
        
        // Save updated dispute case
        save_dispute_case(&env, lease_id, &dispute_case);
        
        // Check if we have enough votes to resolve
        if dispute_case.juror_votes.len() >= JURY_VOTE_THRESHOLD {
            Self::resolve_dispute_with_verdict(env.clone(), lease_id)?;
        }
        
        Ok(())
    }
    
    /// Resolve dispute based on juror verdicts
    fn resolve_dispute_with_verdict(
        env: Env,
        lease_id: u64,
    ) -> Result<(), LeaseError> {
        let mut dispute_case = load_dispute_case(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Count votes (true = favor tenant, false = favor landlord)
        let mut tenant_votes = 0;
        let mut landlord_votes = 0;
        
        for vote in dispute_case.juror_votes.iter() {
            if vote {
                tenant_votes += 1;
            } else {
                landlord_votes += 1;
            }
        }
        
        // Determine verdict (2-of-3 threshold)
        let tenant_wins = tenant_votes >= JURY_VOTE_THRESHOLD;
        
        // Calculate resolution
        let resolution = if tenant_wins {
            // Tenant wins - full refund
            DepositReleasePartial {
                tenant_amount: lease.security_deposit,
                landlord_amount: 0,
            }
        } else {
            // Landlord wins - landlord gets deposit
            DepositReleasePartial {
                tenant_amount: 0,
                landlord_amount: lease.security_deposit,
            }
        };
        
        // Update juror statistics
        for (i, juror_addr) in dispute_case.selected_jurors.iter().enumerate() {
            if i < dispute_case.juror_votes.len() {
                let juror_vote = dispute_case.juror_votes.get(i as u32).unwrap();
                let mut juror_data = load_juror(&env, &juror_addr)
                    .ok_or(LeaseError::JurorNotFound)?;
                
                // Update reputation based on vote alignment with majority
                if (tenant_wins && juror_vote) || (!tenant_wins && !juror_vote) {
                    juror_data.successful_votes += 1;
                    juror_data.reputation += 10;
                } else {
                    juror_data.reputation = juror_data.reputation.saturating_sub(5);
                }
                
                save_juror(&env, &juror_addr, &juror_data);
            }
        }
        
        // Update lease status
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        dispute_case.is_resolved = true;
        dispute_case.resolution = Some(resolution.clone());
        
        // Save updated data
        save_lease_instance(&env, lease_id, &lease);
        save_dispute_case(&env, lease_id, &dispute_case);
        
        // Emit resolution event
        DisputeResolved {
            lease_id,
            resolution,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Handle timeout for juror voting
    pub fn handle_juror_timeout(
        env: Env,
        lease_id: u64,
    ) -> Result<(), LeaseError> {
        let dispute_case = load_dispute_case(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        if dispute_case.is_resolved {
            return Ok(()); // Already resolved
        }
        
        if env.ledger().timestamp() <= dispute_case.verdict_deadline {
            return Err(LeaseError::VerdictDeadlinePassed); // Not yet timed out
        }
        
        // Slash jurors who didn't vote
        for (i, juror_addr) in dispute_case.selected_jurors.iter().enumerate() {
            if i >= dispute_case.juror_votes.len() {
                let mut juror_data = load_juror(&env, &juror_addr)
                    .ok_or(LeaseError::JurorNotFound)?;
                juror_data.stake_amount -= JUROR_SLASH_AMOUNT;
                save_juror(&env, &juror_addr, &juror_data);
                
                JurorSlashed {
                    juror: juror_addr.clone(),
                    slash_amount: JUROR_SLASH_AMOUNT,
                    reason: String::from_str(&env, "Failed to vote on time"),
                }.publish(&env);
            }
        }
        
        // If insufficient votes, forfeit dispute bond to opposing party
        if dispute_case.juror_votes.len() < JURY_VOTE_THRESHOLD {
            let lease = load_lease_instance_by_id(&env, lease_id)
                .ok_or(LeaseError::LeaseNotFound)?;
            
            let opposing_party = if dispute_case.challenger == lease.tenant {
                lease.landlord
            } else {
                lease.tenant
            };
            
            // Transfer dispute bond to opposing party (in a real implementation)
            // This would involve token transfers
            
            // Reset lease to normal disputed state
            let mut lease = load_lease_instance_by_id(&env, lease_id)
                .ok_or(LeaseError::LeaseNotFound)?;
            lease.status = LeaseStatus::Disputed;
            lease.deposit_status = DepositStatus::Disputed;
            save_lease_instance(&env, lease_id, &lease);
        }
        
        Ok(())
    }
    
    // Sub-Leasing Functions
    
    /// Create a sub-lease with hierarchical dependency
    pub fn create_sublease(
        env: Env,
        master_lease_id: u64,
        tenant: Address,
        params: CreateSubleaseParams,
    ) -> Result<u64, LeaseError> {
        tenant.require_auth();
        
        // Verify master lease exists and allows subleasing
        let master_lease = load_lease_instance_by_id(&env, master_lease_id)
            .ok_or(LeaseError::MasterLeaseNotFound)?;
        
        if !master_lease.subleasing_allowed {
            return Err(LeaseError::SubleasingNotAllowed);
        }
        
        // Verify caller is the master lease tenant
        if tenant != master_lease.tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        // Verify sublease duration doesn't exceed master lease duration
        if params.sub_end_date > master_lease.end_date {
            return Err(LeaseError::SubleaseBoundaryExceeded);
        }
        
        if params.sub_start_date < master_lease.start_date {
            return Err(LeaseError::SubleaseBoundaryExceeded);
        }
        
        // Get next sub-lease ID
        let sub_lease_id = get_next_sub_lease_id(&env);
        
        // Create sub-escrow vault
        let vault_id = sub_lease_id; // Use same ID for simplicity
        let sub_escrow_vault = SubEscrowVault {
            master_lease_id,
            sub_lease_id,
            sub_lessee: params.sub_lessee.clone(),
            deposit_amount: params.sub_deposit_amount,
            is_active: true,
            created_at: env.ledger().timestamp(),
        };
        
        // Save sub-escrow vault
        save_sub_escrow_vault(&env, vault_id, &sub_escrow_vault);
        
        // Create sub-lease instance
        let sub_lease = LeaseInstance {
            landlord: tenant, // Master tenant becomes sub-landlord
            tenant: params.sub_lessee.clone(),
            rent_amount: params.sub_rent_amount,
            deposit_amount: params.sub_deposit_amount,
            security_deposit: params.sub_deposit_amount,
            start_date: params.sub_start_date,
            end_date: params.sub_end_date,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            status: LeaseStatus::Pending,
            property_uri: params.property_uri.clone(),
            nft_contract: master_lease.nft_contract.clone(),
            token_id: master_lease.token_id,
            active: true,
            debt: 0,
            rent_paid: 0,
            expiry_time: params.sub_end_date,
            buyout_price: None,
            cumulative_payments: 0,
            rent_per_sec: 0,
            grace_period_end: params.sub_end_date,
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
            inspector: None,
            wear_allowance_bps: master_lease.wear_allowance_bps,
            asset_lifespan_days: master_lease.asset_lifespan_days,
            asset_value: master_lease.asset_value,
            deposit_timestamp: env.ledger().timestamp(),
            subleasing_allowed: false, // Sub-leases cannot be sub-leased further
            master_lease_id: Some(master_lease_id),
        };
        
        // Save sub-lease
        save_lease_instance(&env, sub_lease_id, &sub_lease);
        
        // Emit events
        SubleaseCreated {
            master_lease_id,
            sub_lease_id,
            sub_lessee: params.sub_lessee.clone(),
            sub_escrow_vault_id: vault_id,
        }.publish(&env);
        
        LeaseSigned {
            lease_id: sub_lease_id,
            property_hash: params.property_uri.clone(),
        }.publish(&env);
        
        Ok(sub_lease_id)
    }
    
    /// Handle master lease termination - cascade to all sub-leases
    pub fn terminate_master_with_subleases(
        env: Env,
        master_lease_id: u64,
        caller: Address,
    ) -> Result<(), LeaseError> {
        // First terminate the master lease using existing logic
        Self::terminate_lease(env.clone(), master_lease_id, caller)?;
        
        // Find and terminate all sub-leases
        Self::terminate_all_subleases(env.clone(), master_lease_id, String::from_str(&env, "Master lease terminated"))?;
        
        Ok(())
    }
    
    /// Terminate all sub-leases for a given master lease
    fn terminate_all_subleases(
        env: Env,
        master_lease_id: u64,
        reason: String,
    ) -> Result<(), LeaseError> {
        // This is a simplified implementation
        // In practice, you'd need to iterate through all leases and find sub-leases
        // For now, we'll use a counter-based approach
        
        let mut sub_lease_id = 1;
        let max_lease_id = 10000; // Reasonable upper bound
        
        while sub_lease_id <= max_lease_id {
            if let Some(sub_lease) = load_lease_instance_by_id(&env, sub_lease_id) {
                if let Some(current_master_id) = sub_lease.master_lease_id {
                    if current_master_id == master_lease_id {
                        // Terminate this sub-lease
                        let mut terminated_sublease = sub_lease;
                        terminated_sublease.status = LeaseStatus::Terminated;
                        terminated_sublease.active = false;
                        save_lease_instance(&env, sub_lease_id, &terminated_sublease);
                        
                        // Deactivate sub-escrow vault
                        if let Some(mut vault) = load_sub_escrow_vault(&env, sub_lease_id) {
                            vault.is_active = false;
                            save_sub_escrow_vault(&env, sub_lease_id, &vault);
                        }
                        
                        SubleaseTerminated {
                            master_lease_id,
                            sub_lease_id,
                            reason: reason.clone(),
                        }.publish(&env);
                    }
                }
            }
            sub_lease_id += 1;
        }
        
        Ok(())
    }
    
    /// Handle sub-lease damage - slash sub-escrow first, then master deposit
    pub fn handle_sublease_damage(
        env: Env,
        sub_lease_id: u64,
        damage_amount: i128,
    ) -> Result<(), LeaseError> {
        let sub_lease = load_lease_instance_by_id(&env, sub_lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        let master_lease_id = sub_lease.master_lease_id
            .ok_or(LeaseError::MasterLeaseNotFound)?;
        
        let mut sub_escrow_vault = load_sub_escrow_vault(&env, sub_lease_id)
            .ok_or(LeaseError::SubEscrowVaultNotFound)?;
        
        if !sub_escrow_vault.is_active {
            return Err(LeaseError::Unauthorised);
        }
        
        // First, try to cover damage from sub-escrow
        let remaining_damage = if damage_amount <= sub_escrow_vault.deposit_amount {
            sub_escrow_vault.deposit_amount -= damage_amount;
            0 // Damage fully covered
        } else {
            let remaining = damage_amount - sub_escrow_vault.deposit_amount;
            sub_escrow_vault.deposit_amount = 0;
            remaining // Remaining damage to be covered by master lease
        };
        
        // Save updated sub-escrow vault
        save_sub_escrow_vault(&env, sub_lease_id, &sub_escrow_vault);
        
        // If there's remaining damage, charge against master lease deposit
        if remaining_damage > 0 {
            let mut master_lease = load_lease_instance_by_id(&env, master_lease_id)
                .ok_or(LeaseError::MasterLeaseNotFound)?;
            
            // Ensure we don't exceed master deposit
            let actual_damage = remaining_damage.min(master_lease.security_deposit);
            master_lease.security_deposit -= actual_damage;
            
            save_lease_instance(&env, master_lease_id, &master_lease);
        }
        
        Ok(())
    }
    
    /// Get sub-lease hierarchy information
    pub fn get_sublease_hierarchy(
        env: Env,
        lease_id: u64,
    ) -> Result<(Option<u64>, soroban_sdk::Vec<u64>), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        let master_lease_id = lease.master_lease_id;
        let mut sub_leases = soroban_sdk::Vec::new(&env);
        
        // Find all sub-leases of this lease (simplified implementation)
        let mut potential_sub_id = 1;
        let max_lease_id = 10000;
        
        while potential_sub_id <= max_lease_id {
            if let Some(potential_sub) = load_lease_instance_by_id(&env, potential_sub_id) {
                if let Some(potential_master_id) = potential_sub.master_lease_id {
                    if potential_master_id == lease_id {
                        sub_leases.push_back(potential_sub_id);
                    }
                }
            }
            potential_sub_id += 1;
        }
        
        Ok((master_lease_id, sub_leases))
    }
    
    /// Validate sub-lease boundaries recursively
    pub fn validate_sublease_boundaries(
        env: Env,
        sub_lease_id: u64,
    ) -> Result<bool, LeaseError> {
        let sub_lease = load_lease_instance_by_id(&env, sub_lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        let master_lease_id = sub_lease.master_lease_id
            .ok_or(LeaseError::MasterLeaseNotFound)?;
        
        let master_lease = load_lease_instance_by_id(&env, master_lease_id)
            .ok_or(LeaseError::MasterLeaseNotFound)?;
        
        // Check temporal boundaries
        if sub_lease.start_date < master_lease.start_date ||
           sub_lease.end_date > master_lease.end_date {
            return Ok(false);
        }
        
        // Recursively check master lease boundaries
        if let Some(grand_master_id) = master_lease.master_lease_id {
            Self::validate_sublease_boundaries(env, master_lease_id)?;
        }
        
        Ok(true)
    }
}

mod test;
mod upgrade_tests;
