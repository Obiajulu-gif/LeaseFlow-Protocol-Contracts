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
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub paused_at: Option<u64>,
    pub pause_initiator: Option<Address>,
    pub total_paused_duration: u64,
    pub rent_pull_authorized_amount: Option<i128>,
    pub last_rent_pull_timestamp: Option<u64>,
    pub billing_cycle_duration: u64,
    pub yield_delegation_enabled: bool,
    pub yield_accumulated: i128,
    pub equity_balance: i128,
    pub equity_percentage_bps: u32,
    pub had_late_payment: bool,
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    pub early_termination_fee_bps: Option<u32>,
    pub fixed_penalty: Option<i128>,
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
    pub early_termination_fee_bps: Option<u32>,
    pub fixed_penalty: Option<i128>,
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
    DefaultedBalance(Address),
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
pub struct EarlyTerminationExecuted {
    pub lease_id: u64,
    pub tenant: Address,
    pub landlord: Address,
    pub penalty_amount: i128,
    pub remaining_deposit: i128,
    pub duration_remaining: u64,
    pub total_lease_duration: u64,
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
        ) -> Result<i128, i32>;
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
            dex_client
                .path_payment(
                    tenant.clone(),
                    collateral_asset.clone(),
                    original_amount,
                    max_slippage_bps,
                    swap_path.clone(),
                )
                .map_err(|_| LeaseError::PathPaymentFailed)?
        } else {
            let simulated_output = original_amount.saturating_mul(9_900) / 10_000;
            let min_out = original_amount.saturating_mul(10_000i128 - max_slippage_bps as i128)
                / 10_000i128;
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
            early_termination_fee_bps: params.early_termination_fee_bps,
            fixed_penalty: params.fixed_penalty,
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

    pub fn execute_early_termination(
        env: Env,
        lease_id: u64,
        tenant: Address,
    ) -> Result<(), LeaseError> {
        tenant.require_auth();

        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Only tenant can execute early termination
        if lease.tenant != tenant {
            return Err(LeaseError::Unauthorised);
        }

        // Lease must be active
        if !lease.active || lease.status != LeaseStatus::Active {
            return Err(LeaseError::LeaseNotFound);
        }

        let current_time = env.ledger().timestamp();

        // Cannot terminate after end date (use normal termination)
        if current_time >= lease.end_date {
            return Err(LeaseError::LeaseNotExpired);
        }

        // Flash loan protection: Check tenant's balance before and after
        // This prevents tenants from using flash loans to temporarily inflate balances
        let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
        let tenant_balance_before = token_client.balance(&tenant);

        // Require minimum balance to prevent flash loan manipulation
        // Tenant must have at least the security deposit amount naturally
        if tenant_balance_before < lease.security_deposit {
            return Err(LeaseError::Unauthorised);
        }

        let total_lease_duration = lease.end_date - lease.start_date;
        let elapsed_duration = current_time - lease.start_date;
        let duration_remaining = lease.end_date - current_time;

        // Calculate remaining unpaid lease value
        let remaining_lease_value = if lease.rent_per_sec > 0 {
            (duration_remaining as i128).saturating_mul(lease.rent_per_sec)
        } else {
            0
        };

        // Calculate penalty based on lease configuration
        let penalty_amount = match (lease.early_termination_fee_bps, lease.fixed_penalty) {
            (Some(fee_bps), _) => {
                // Percentage-based penalty on remaining lease value
                remaining_lease_value.saturating_mul(fee_bps as i128) / 10_000
            }
            (None, Some(fixed_amount)) => {
                // Fixed penalty amount
                fixed_amount
            }
            (None, None) => {
                // No penalty configured
                0
            }
        };

        // Ensure penalty doesn't exceed security deposit
        let final_penalty = penalty_amount.min(lease.security_deposit);
        let remaining_deposit = lease.security_deposit - final_penalty;

        // Handle case where penalty exceeds entire deposit
        if penalty_amount > lease.security_deposit {
            // Flag tenant as defaulted
            let defaulted_key = DataKey::DefaultedBalance(tenant.clone());
            env.storage().persistent().set(&defaulted_key, &true);
            env.storage()
                .persistent()
                .extend_ttl(&defaulted_key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
        }

        // Transfer penalty to landlord
        if final_penalty > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.landlord,
                &final_penalty,
            );
        }

        // Transfer remaining deposit back to tenant
        if remaining_deposit > 0 {
            let token_client = token_contract::TokenClient::new(&env, &lease.payment_token);
            token_client.transfer(
                &env.current_contract_address(),
                &lease.tenant,
                &remaining_deposit,
            );
        }

        // Flash loan protection: Verify tenant still has sufficient balance after operations
        let tenant_balance_after = token_client.balance(&tenant);

        // Tenant should still have a reasonable buffer after penalty deduction
        // This prevents flash loan attacks where temporary funds are used and immediately withdrawn
        let min_buffer = lease.security_deposit / 10; // 10% buffer
        if tenant_balance_after < min_buffer && remaining_deposit > 0 {
            // This could indicate flash loan manipulation, revert the transaction
            return Err(LeaseError::Unauthorised);
        }

        // Update lease status
        lease.status = LeaseStatus::Terminated;
        lease.active = false;
        lease.deposit_status = DepositStatus::Settled;

        // Handle NFT return if present
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

        // Archive the lease
        archive_lease(&env, lease_id, lease.clone(), tenant.clone());

        // Emit event
        EarlyTerminationExecuted {
            lease_id,
            tenant: tenant.clone(),
            landlord: lease.landlord.clone(),
            penalty_amount: final_penalty,
            remaining_deposit,
            duration_remaining,
            total_lease_duration,
        }.publish(&env);

        Ok(())
    }
}

mod test;
mod upgrade_tests;
mod early_termination_tests;
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
pub struct OraclePayload {
    pub lease_id: u64,
    pub oracle_pubkey: BytesN<32>,
    pub damage_severity: DamageSeverity,
    pub nonce: u64,
    pub timestamp: u64,
    pub signature: BytesN<64>,
}

#[contracttype]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetTier {
    Low,      // Basic assets, low deposit requirements
    Medium,   // Standard assets, moderate deposit requirements
    High,     // Premium assets, high deposit requirements
    Luxury,   // Luxury assets, very high deposit requirements
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowVault {
    pub total_locked: i128,
    pub lease_count: u64,
    pub max_capacity: i128,
    pub supported_assets: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityDeposit {
    pub lease_id: u64,
    pub lessee: Address,
    pub lessor: Address,
    pub asset_address: Address,
    pub amount: i128,
    pub locked_at: u64,
    pub status: DepositStatus,
    pub asset_tier: AssetTier,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiAssetCollateral {
    pub primary_asset: Address,
    pub primary_amount: i128,
    pub secondary_asset: Option<Address>,
    pub secondary_amount: Option<i128>,
    pub nft_contract: Option<Address>,
    pub nft_token_id: Option<u128>,
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
    pub yield_delegation_enabled: bool,
    pub yield_accumulated: i128,
    pub equity_balance: i128,
    pub equity_percentage_bps: u32,
    pub had_late_payment: bool,
    pub has_pet: bool,
    pub pet_deposit_amount: i128,
    pub pet_rent_amount: i128,
    pub last_tenant_interaction: u64,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseRenewalProposal {
    pub lease_id: u64,
    pub landlord: Address,
    pub proposed_end_date: u64,
    pub proposed_rent_amount: i128,
    pub proposed_deposit_amount: i128,
    pub proposed_rent_per_sec: i128,
    pub expiration_timestamp: u64,
    pub landlord_signature: bool,
    pub tenant_signature: bool,
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
    LeaseRenewalProposal(u64),
    WhitelistedOracle(BytesN<32>),
    OracleNonce(BytesN<32>, u64),
    TenantFlag(u64),
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
pub struct LeaseRenewed {
    pub old_lease_id: u64,
    pub new_duration: u64,
    pub rolled_over_deposit: i128,
    pub extension_amount: i128,
pub struct DepositSeizedForAbandonment {
    pub lease_id: u64,
    pub landlord: Address,
    pub tenant: Address,
    pub deposit_amount: i128,
    pub seizure_timestamp: u64,
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
    RenewalConsensusFailed = 19,
    RenewalNotProposed = 20,
    RenewalExpired = 21,
    InvalidRenewalTerms = 22,
    AbandonmentChallenge = 19,
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

pub fn save_renewal_proposal(env: &Env, lease_id: u64, proposal: &LeaseRenewalProposal) {
    let key = DataKey::LeaseRenewalProposal(lease_id);
    env.storage().persistent().set(&key, proposal);
// Dispute resolution helper functions
pub fn save_dispute_case(env: &Env, dispute_id: u64, dispute_case: &DisputeCase) {
    let key = DataKey::DisputeCase(dispute_id);
    env.storage().persistent().set(&key, dispute_case);
    env.storage()
        .persistent()
        .extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_renewal_proposal(env: &Env, lease_id: u64) -> Option<LeaseRenewalProposal> {
    env.storage()
        .persistent()
        .get(&DataKey::LeaseRenewalProposal(lease_id))
}

pub fn delete_renewal_proposal(env: &Env, lease_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::LeaseRenewalProposal(lease_id));
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
        ) -> Result<i128, i32>;
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
            dex_client
                .path_payment(
                    tenant.clone(),
                    collateral_asset.clone(),
                    original_amount,
                    max_slippage_bps,
                    swap_path.clone(),
                )
                .map_err(|_| LeaseError::PathPaymentFailed)?
        } else {
            let simulated_output = original_amount.saturating_mul(9_900) / 10_000;
            let min_out = original_amount.saturating_mul(10_000i128 - max_slippage_bps as i128)
                / 10_000i128;
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
        
        // Calculate required security deposit based on asset tier and duration
        let lease_duration = params.end_date.saturating_sub(params.start_date);
        let asset_tier = Self::get_asset_tier(&env, &params.payment_token);
        let required_deposit = Self::calculate_required_deposit(
            &env,
            asset_tier,
            lease_duration,
            params.security_deposit,
        );
        
        // Verify that the provided security deposit matches the required amount
        if params.security_deposit != required_deposit {
            return Err(LeaseError::InvalidDeduction);
        }
        
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
            last_tenant_interaction: env.ledger().timestamp(),
        };
        save_lease_instance(&env, lease_id, &lease);
        LeaseSigned {
            lease_id,
            property_hash: params.property_uri.clone(),
        }
        .publish(&env);
        Ok(())
    }

    fn update_tenant_heartbeat(env: &Env, lease_id: u64, tenant: &Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.tenant != *tenant {
            return Err(LeaseError::Unauthorised);
        }
        lease.last_tenant_interaction = env.ledger().timestamp();
        save_lease_instance(env, lease_id, &lease);
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

        // Update tenant heartbeat if this is the tenant paying
        if is_primary {
            update_tenant_heartbeat(&env, lease_id, &payer)?;
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
        
        // Update tenant heartbeat
        update_tenant_heartbeat(&env, lease_id, &tenant)?;
        
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

        // Verify that both parties are the actual lease participants
        if lessee_pubkey != lease.tenant || lessor_pubkey != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }

        // Verify both parties have authorized this transaction
        lessee_pubkey.require_auth();
        lessor_pubkey.require_auth();

        // Validate lease is in a state that allows mutual release
        if lease.status != LeaseStatus::Active && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotFound);
        }

        // Mathematical validation: ensure amounts sum to total escrowed deposit
        let total_escrowed = lease.security_deposit + lease.deposit_amount;
        if return_amount + slash_amount != total_escrowed {
            return Err(LeaseError::InvalidReleaseMath);
        }

        // Ensure amounts are non-negative
        if return_amount < 0 || slash_amount < 0 {
            return Err(LeaseError::InvalidReleaseMath);
        }

        // Calculate the split: return_amount goes to tenant, slash_amount goes to landlord
        let tenant_refund = return_amount;
        let landlord_payout = slash_amount;

        // Execute atomic transfers
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

        // Update lease state to finalized
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;

        // Handle NFT return if applicable
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

        // Emit the mutual lease finalized event
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

    pub fn initiate_mutual_release_with_fallback(
        env: Env,
        lease_id: u64,
        initiator_pubkey: Address,
        proposed_return_amount: i128,
        proposed_slash_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease =
            load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Verify initiator is a lease participant
        if initiator_pubkey != lease.tenant && initiator_pubkey != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }

        initiator_pubkey.require_auth();

        // Validate lease is in a state that allows mutual release attempt
        if lease.status != LeaseStatus::Active && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotFound);
        }

        // Mathematical validation of proposed amounts
        let total_escrowed = lease.security_deposit + lease.deposit_amount;
        if proposed_return_amount + proposed_slash_amount != total_escrowed {
            return Err(LeaseError::InvalidReleaseMath);
        }

        // Ensure amounts are non-negative
        if proposed_return_amount < 0 || proposed_slash_amount < 0 {
            return Err(LeaseError::InvalidReleaseMath);
        }

        // In a real implementation, this would store the proposal and wait for the counterparty
        // For now, we immediately transition to dispute state to demonstrate the fallback
        lease.deposit_status = DepositStatus::Disputed;
        lease.status = LeaseStatus::Disputed;

        save_lease_instance(&env, lease_id, &lease);

        // Emit dispute event to trigger Oracle intervention
        DepositDisputed { 
            lease_id, 
            caller: initiator_pubkey 
        }.publish(&env);

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

    pub fn claim_abandoned_deposit(
        env: Env,
        lease_id: u64,
        landlord: Address,
    ) -> Result<(), LeaseError> {
        landlord.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        // Only landlord can claim abandoned deposit
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        
        // Lease must be in Expired status
        if lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotExpired);
        }
        
        let current_time = env.ledger().timestamp();
        const ABANDONMENT_GRACE_PERIOD: u64 = 30 * 24 * 60 * 60; // 30 days in seconds
        
        // Check if lease has been expired for at least 30 days
        if lease.end_date + ABANDONMENT_GRACE_PERIOD > current_time {
            return Err(LeaseError::LeaseNotExpired);
        }
        
        // Check if tenant has any interaction within the 30-day window
        let abandonment_deadline = lease.end_date + ABANDONMENT_GRACE_PERIOD;
        if lease.last_tenant_interaction > abandonment_deadline {
            return Err(LeaseError::AbandonmentChallenge);
        }
        
        // All conditions met - seize the deposit
        let deposit_amount = lease.security_deposit;
        
        // Update lease status and deposit status
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;
        
        save_lease_instance(&env, lease_id, &lease);
        
        // Emit event for public record
        DepositSeizedForAbandonment {
            lease_id,
            landlord: lease.landlord.clone(),
            tenant: lease.tenant.clone(),
            deposit_amount,
            seizure_timestamp: current_time,
        }
        .publish(&env);
        
        // Archive the lease to maintain ledger hygiene
        archive_lease(&env, lease_id, lease, landlord);
        
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

    pub fn propose_lease_renewal(
        env: Env,
        lease_id: u64,
        landlord: Address,
        proposed_end_date: u64,
        proposed_rent_amount: i128,
        proposed_deposit_amount: i128,
        proposed_rent_per_sec: i128,
        proposal_duration: u64,
    ) -> Result<(), LeaseError> {
        landlord.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Authorization checks
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        
        // Lease must be active to be renewed
        if lease.status != LeaseStatus::Active || !lease.active {
            return Err(LeaseError::InvalidRenewalTerms);
        }
        
        // Validation checks
        let current_time = env.ledger().timestamp();
        if proposed_end_date <= lease.end_date || proposed_end_date <= current_time {
            return Err(LeaseError::InvalidRenewalTerms);
        }
        
        if proposed_rent_amount <= 0 || proposed_deposit_amount <= 0 || proposed_rent_per_sec <= 0 {
            return Err(LeaseError::InvalidRenewalTerms);
        }
        
        // Remove any existing proposal for this lease
        delete_renewal_proposal(&env, lease_id);
        
        // Create new proposal
        let proposal = LeaseRenewalProposal {
            lease_id,
            landlord: lease.landlord.clone(),
            proposed_end_date,
            proposed_rent_amount,
            proposed_deposit_amount,
            proposed_rent_per_sec,
            expiration_timestamp: current_time.saturating_add(proposal_duration),
            landlord_signature: true,
            tenant_signature: false,
        };
        
        save_renewal_proposal(&env, lease_id, &proposal);
        Ok(())
    }
    
    pub fn accept_renewal(
        env: Env,
        lease_id: u64,
        tenant: Address,
    ) -> Result<(), LeaseError> {
        tenant.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Authorization checks
        if lease.tenant != tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        // Load and validate proposal
        let mut proposal = load_renewal_proposal(&env, lease_id)
            .ok_or(LeaseError::RenewalNotProposed)?;
        
        // Check if proposal has expired
        if env.ledger().timestamp() > proposal.expiration_timestamp {
            delete_renewal_proposal(&env, lease_id);
            return Err(LeaseError::RenewalExpired);
        }
        
        // Verify landlord has signed
        if !proposal.landlord_signature {
            return Err(LeaseError::RenewalConsensusFailed);
        }
        
        // Add tenant signature
        proposal.tenant_signature = true;
        
        // Both parties have signed - execute renewal
        let current_time = env.ledger().timestamp();
        let old_end_date = lease.end_date;
        let old_deposit = lease.security_deposit;
        let new_duration = proposal.proposed_end_date.saturating_sub(current_time);
        
        // Calculate deposit differences
        let deposit_difference = proposal.proposed_deposit_amount.saturating_sub(old_deposit);
        let refund_amount = old_deposit.saturating_sub(proposal.proposed_deposit_amount);
        
        // Handle deposit adjustments
        if deposit_difference > 0 {
            // Tenant needs to provide additional deposit
            // This would require token transfer logic - for now, we'll assume it's handled
            // In a full implementation, this would pull tokens from tenant
            lease.security_deposit = proposal.proposed_deposit_amount;
        } else if refund_amount > 0 {
            // Refund excess deposit to tenant
            // This would require token transfer logic - for now, we'll assume it's handled
            // In a full implementation, this would transfer tokens to tenant
            lease.security_deposit = proposal.proposed_deposit_amount;
        }
        
        // Update lease terms
        lease.end_date = proposal.proposed_end_date;
        lease.expiry_time = proposal.proposed_end_date;
        lease.rent_amount = proposal.proposed_rent_amount;
        lease.rent_per_sec = proposal.proposed_rent_per_sec;
        lease.grace_period_end = proposal.proposed_end_date;
        
        // Preserve accumulated yield and other state
        // Time-based proration logic continues seamlessly
        
        // Save updated lease
        save_lease_instance(&env, lease_id, &lease);
        
        // Clean up proposal
        delete_renewal_proposal(&env, lease_id);
        
        // Emit event
        LeaseRenewed {
            old_lease_id: lease_id,
            new_duration,
            rolled_over_deposit: lease.security_deposit,
            extension_amount: proposal.proposed_end_date.saturating_sub(old_end_date),
        }.publish(&env);
        
        Ok(())
    }
    
    pub fn reject_renewal(
        env: Env,
        lease_id: u64,
        party: Address,
    ) -> Result<(), LeaseError> {
        party.require_auth();
        
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Verify caller is either landlord or tenant
        if party != lease.landlord && party != lease.tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        // Remove proposal
        delete_renewal_proposal(&env, lease_id);
        
        Ok(())
    }
    
    pub fn get_renewal_proposal(env: Env, lease_id: u64) -> Result<LeaseRenewalProposal, LeaseError> {
        load_renewal_proposal(&env, lease_id)
            .ok_or(LeaseError::RenewalNotProposed)
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

    fn verify_ed25519_signature(
        env: &Env,
        pubkey: &BytesN<32>,
        message: &soroban_sdk::Bytes,
        signature: &BytesN<64>,
    ) -> bool {
        env.crypto().ed25519_verify(pubkey, message, signature)
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::OracleNonce(oracle_pubkey.clone(), 0), YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
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
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::TenantFlag(lease_id), YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        TenantFlagged {
            lease_id,
            tenant,
            reason,
        }
        .publish(env);
    }

    pub fn execute_deposit_slash(
        env: Env,
        payload: OraclePayload,
    ) -> Result<(), LeaseError> {
        let current_time = env.ledger().timestamp();
        
        if !Self::is_oracle_whitelisted(&env, &payload.oracle_pubkey) {
            return Err(LeaseError::OracleNotWhitelisted);
        }

        let stored_nonce = Self::get_oracle_nonce(&env, &payload.oracle_pubkey);
        if payload.nonce <= stored_nonce {
            return Err(LeaseError::InvalidNonce);
        }

        if payload.timestamp > current_time || current_time - payload.timestamp > 86400 {
            return Err(LeaseError::InvalidSignature);
        }

        let mut message_data = soroban_sdk::Bytes::new(&env);
        message_data.append(&payload.lease_id.to_val());
        message_data.append(&payload.oracle_pubkey.to_val());
        message_data.append(&(payload.damage_severity as u32).to_val());
        message_data.append(&payload.nonce.to_val());
        message_data.append(&payload.timestamp.to_val());

        if !Self::verify_ed25519_signature(&env, &payload.oracle_pubkey, &message_data, &payload.signature) {
            return Err(LeaseError::InvalidSignature);
        }

        Self::set_oracle_nonce(&env, &payload.oracle_pubkey, payload.nonce);

        let mut lease = load_lease_instance_by_id(&env, payload.lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        if lease.status != LeaseStatus::Terminated && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotTerminated);
        }

        if lease.deposit_status == DepositStatus::Settled {
            return Err(LeaseError::DepositAlreadySettled);
        }

        let total_deposit = lease.security_deposit + lease.deposit_amount;
        let penalty_percentage = Self::calculate_penalty_percentage(payload.damage_severity);
        let penalty_amount = if penalty_percentage == 0 {
            0
        } else {
            total_deposit.saturating_mul(penalty_percentage as i128) / 100
        };

        let tenant_refund = total_deposit.saturating_sub(penalty_amount);
        let landlord_payout = penalty_amount;

        if payload.damage_severity as u32 >= DamageSeverity::Severe as u32 && penalty_amount >= total_deposit {
            Self::flag_tenant(&env, payload.lease_id, lease.tenant.clone(), 
                String::from_str(&env, "Severe damage exceeding deposit value"));
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
}

mod test;
mod upgrade_tests;
mod renewal_tests;
#[cfg(test)]
mod security_deposit_tests;
