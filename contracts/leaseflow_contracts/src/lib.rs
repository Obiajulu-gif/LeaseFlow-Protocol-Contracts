#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, 
    Address, Env, String, Symbol, BytesN
};

// ── Enums ──────────────────────────────────────────────────────────────

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
    pub payment_token: Address,
    pub active: bool,
    pub rent_paid: i128,
    pub expiry_time: u64,
    /// Optional price at which the tenant can buy out the asset.
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
    pub maintenance_status: MaintenanceStatus,
    pub withheld_rent: i128,
    pub inspector: Option<Address>,
    pub repair_proof_hash: Option<BytesN<32>>,
    /// Pre-approved destination for landlord's rent withdrawals.
    pub withdrawal_address: Option<Address>,
    /// Total rent withdrawn by the landlord.
    pub rent_withdrawn: i128,
    /// Whitelisted arbitrators agreed upon by both parties.
    pub arbitrators: soroban_sdk::Vec<Address>,
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
    pub arbitrators: soroban_sdk::Vec<Address>,
    pub rent_per_sec: i128,
    pub grace_period_end: u64,
    pub late_fee_flat: i128,
    pub late_fee_per_sec: i128,
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
    WithdrawalAddressMismatch = 14,
    PaymentTokenMismatch = 15,
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
        RateType::PerHour   => rate / 3_600,
        RateType::PerDay    => rate / 86_400,
    }
}

pub fn save_lease(env: &Env, lease_id: &Symbol, lease: &Lease) {
    let key = DataKey::Lease(lease_id.clone());
    env.storage().persistent().set(&key, lease);
    env.storage().persistent().extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_lease_by_id(env: &Env, lease_id: &Symbol) -> Option<Lease> {
    env.storage().persistent().get(&DataKey::Lease(lease_id.clone()))
}

pub fn save_lease_instance(env: &Env, lease_id: u64, lease: &LeaseInstance) {
    let key = DataKey::LeaseInstance(lease_id);
    env.storage().persistent().set(&key, lease);
    env.storage().persistent().extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

pub fn load_lease_instance_by_id(env: &Env, lease_id: u64) -> Option<LeaseInstance> {
    env.storage().persistent().get(&DataKey::LeaseInstance(lease_id))
}

pub fn delete_lease_instance(env: &Env, lease_id: u64) {
    env.storage().persistent().remove(&DataKey::LeaseInstance(lease_id));
}

pub fn save_usage_rights(env: &Env, nft_contract: Address, token_id: u128, usage_rights: &UsageRights) {
    env.storage().instance().set(&DataKey::UsageRights(nft_contract, token_id), usage_rights);
}

pub fn delete_usage_rights(env: &Env, nft_contract: Address, token_id: u128) {
    env.storage().instance().remove(&DataKey::UsageRights(nft_contract, token_id));
}

pub fn load_usage_rights(env: &Env, nft_contract: Address, token_id: u128) -> Option<UsageRights> {
    env.storage().instance().get(&DataKey::UsageRights(nft_contract, token_id))
}

pub fn archive_lease(env: &Env, lease_id: u64, lease: LeaseInstance, caller: Address) {
    let historical = HistoricalLease {
        lease,
        terminated_by: caller,
        terminated_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&DataKey::HistoricalLease(lease_id), &historical);
    delete_lease_instance(env, lease_id);
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
        // Enforce specific stablecoin assets (USDC, ARST, etc.)
        // For institutional adoption, we check against a curated list of allowed assets.

        // Let's assume there is a specific storage key for allowed assets.
        // If it's not present, and it's not one of our hardcoded "trusted" ones:
        if !Self::is_asset_allowed(env, token) {
            return Err(LeaseError::InvalidAsset);
        }
        Ok(())
    }

    fn is_asset_allowed(env: &Env, token: &Address) -> bool {
        env.storage().instance().has(&DataKey::AllowedAsset(token.clone()))
    }

    pub fn add_allowed_asset(env: Env, admin: Address, asset: Address) -> Result<(), LeaseError> {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin { return Err(LeaseError::Unauthorised); }
        admin.require_auth();
        env.storage().instance().set(&DataKey::AllowedAsset(asset), &true);
        Ok(())
    }


    fn require_kyc(env: &Env, landlord: &Address, tenant: &Address) -> Result<(), LeaseError> {

        if let Some(provider_addr) = env.storage().instance().get::<_, Address>(&DataKey::KycProvider) {
            let client = kyc_contract::KycClient::new(env, &provider_addr);
            if !client.is_verified(landlord) || !client.is_verified(tenant) {
                return Err(LeaseError::KycRequired);
            }
        }
        Ok(())
    }

    pub fn set_kyc_provider(env: Env, admin: Address, provider: Address) -> Result<(), LeaseError> {
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(LeaseError::Unauthorised)?;
        if admin != stored_admin { return Err(LeaseError::Unauthorised); }
        admin.require_auth();
        env.storage().instance().set(&DataKey::KycProvider, &provider);
        Ok(())
    }

    // --- SIMPLE LEASE (Symbol-based) ---


    pub fn initialize_lease(env: Env, lease_id: Symbol, landlord: Address, tenant: Address, rent_amount: i128, deposit_amount: i128, duration: u64, property_uri: String, payment_token: Address) -> Result<bool, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        let start_date = env.ledger().timestamp();
        let end_date = start_date.saturating_add(duration);
        let lease = Lease {
            landlord, tenant, rent_per_sec: 0, late_fee_per_sec: 0, deposit_amount, start_date, end_date, property_uri, status: LeaseStatus::Pending, nft_contract: None, token_id: None, active: true, grace_period_end: end_date, late_fee_flat: 0, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time: end_date, buyout_price: None, cumulative_payments: 0, payment_token,
        };
        env.storage().instance().set(&lease_id, &lease);
        Ok(true)
    }



    pub fn create_lease(env: Env, landlord: Address, tenant: Address, _amount: i128, payment_token: Address) -> Result<Symbol, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        let lease_id = symbol_short!("lease");
        let lease = Lease {
            landlord, tenant, rent_per_sec: 0, late_fee_per_sec: 0, deposit_amount: 0, start_date: env.ledger().timestamp(), end_date: 0, property_uri: String::from_str(&env, ""), status: LeaseStatus::Pending, nft_contract: None, token_id: None, active: true, grace_period_end: 0, late_fee_flat: 0, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time: 0, buyout_price: None, cumulative_payments: 0, payment_token,
        };
        env.storage().instance().set(&lease_id, &lease);
        Ok(lease_id)
    }



    pub fn create_lease_with_nft(env: Env, lease_id: Symbol, landlord: Address, tenant: Address, rent_amount: i128, rent_rate_type: RateType, duration: u64, grace_period_end: u64, late_fee_flat: i128, late_fee_amount: i128, late_fee_rate_type: RateType, nft_contract_addr: Address, token_id: u128, payment_token: Address) -> Result<Symbol, LeaseError> {
        landlord.require_auth();
        Self::require_kyc(&env, &landlord, &tenant)?;
        Self::require_stablecoin(&env, &payment_token)?;
        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        nft_client.transfer_from(&env.current_contract_address(), &landlord, &env.current_contract_address(), &token_id);
        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);
        let lease = Lease {
            landlord, tenant: tenant.clone(), rent_per_sec: to_per_second(rent_amount, rent_rate_type), late_fee_per_sec: to_per_second(late_fee_amount, late_fee_rate_type), deposit_amount: 0, start_date: now, end_date: expiry_time, property_uri: String::from_str(&env, ""), status: LeaseStatus::Active, nft_contract: Some(nft_contract_addr.clone()), token_id: Some(token_id), active: true, grace_period_end, late_fee_flat, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time, buyout_price: None, cumulative_payments: 0, payment_token,
        };
        save_usage_rights(&env, nft_contract_addr, token_id, &UsageRights { renter: tenant, nft_contract: lease.nft_contract.clone().unwrap(), token_id, lease_id: lease_id.clone(), valid_until: expiry_time });
        env.storage().instance().set(&lease_id, &lease);
        Ok(symbol_short!("created"))
    }



    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.tenant == tenant, "Unauthorized");
        lease.status = LeaseStatus::Active;
        env.storage().instance().set(&lease_id, &lease);
        LeaseStarted { id: env.ledger().timestamp(), renter: tenant, rate: lease.rent_per_sec }.publish(&env);
        symbol_short!("active")
    }

    pub fn pay_rent(env: Env, lease_id: Symbol, payment_amount: i128) -> Result<Symbol, LeaseError> {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.active, "Lease is not active");
        Self::require_kyc(&env, &lease.landlord, &lease.tenant)?;
        Self::require_stablecoin(&env, &lease.payment_token)?;
        lease.cumulative_payments += payment_amount;

        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id) {
                    let nft_client = nft_contract::NftClient::new(&env, nft_contract);
                    nft_client.transfer_from(&env.current_contract_address(), &env.current_contract_address(), &lease.tenant, token_id);
                }
            }
        }
        env.storage().instance().set(&lease_id, &lease);
        Ok(symbol_short!("paid"))
    }


    pub fn pay_rent_receipt(env: Env, lease_id: Symbol, month: u32, amount: i128) -> bool {
        let receipt = Receipt { lease_id, month, amount, date: env.ledger().timestamp() };
        env.storage().instance().set(&DataKey::Receipt(receipt.lease_id.clone(), month), &receipt);
        true
    }

    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage().instance().get(&lease_id).expect("Lease not found")
    }

    pub fn get_lease_default(env: Env) -> Lease {
        env.storage().instance().get(&symbol_short!("lease")).expect("Lease not found")
    }

    pub fn set_buyout_price(env: Env, lease_id: Symbol, landlord: Address, buyout_price: i128) -> Symbol {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.landlord == landlord, "Unauthorized");
        lease.buyout_price = Some(buyout_price);
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("buyout")
    }

    pub fn get_receipt(env: Env, lease_id: Symbol, month: u32) -> Receipt {
        env.storage().instance().get(&DataKey::Receipt(lease_id, month)).expect("Receipt not found")
    }

    pub fn end_lease(env: Env, lease_id: Symbol, caller: Address) -> Symbol {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.landlord == caller || lease.tenant == caller, "Unauthorized");
        caller.require_auth();
        if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id) {
            delete_usage_rights(&env, nft_contract.clone(), *token_id);
            let nft_client = nft_contract::NftClient::new(&env, nft_contract);
            nft_client.transfer_from(&env.current_contract_address(), &env.current_contract_address(), &lease.landlord, token_id);
        }
        lease.status = LeaseStatus::Terminated;
        lease.active = false;
        env.storage().instance().set(&lease_id, &lease);
        LeaseEnded { id: env.ledger().timestamp(), duration: env.ledger().timestamp() - lease.start_date, total_paid: lease.cumulative_payments }.publish(&env);
        symbol_short!("ended")
    }

    pub fn extend_ttl(env: Env, _lease_id: Symbol) {
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
    }

    pub fn check_usage_rights(env: Env, nft_contract: Address, token_id: u128, user: Address) -> Option<UsageRights> {
        if let Some(rights) = load_usage_rights(&env, nft_contract, token_id) {
            if rights.renter == user && env.ledger().timestamp() <= rights.valid_until { return Some(rights); }
        }
        None
    }

    // --- LEASE INSTANCE (u64-based) ---

    pub fn create_lease_instance(env: Env, lease_id: u64, landlord: Address, params: CreateLeaseParams) -> Result<(), LeaseError> {
        landlord.require_auth();
        // Require tenant agreement to the terms, including the arbitrator whitelist
        params.tenant.require_auth();

        Self::require_kyc(&env, &landlord, &params.tenant)?;
        Self::require_stablecoin(&env, &params.payment_token)?;

        let lease = LeaseInstance {
            landlord,
            tenant: params.tenant,
            rent_amount: params.rent_amount,
            deposit_amount: params.deposit_amount,
            security_deposit: params.security_deposit,
            start_date: params.start_date,
            end_date: params.end_date,
            property_uri: params.property_uri,
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            payment_token: params.payment_token,
            active: true,
            rent_paid: 0,
            expiry_time: params.end_date,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            rent_per_sec: params.rent_per_sec,
            grace_period_end: params.grace_period_end,
            late_fee_flat: params.late_fee_flat,
            late_fee_per_sec: params.late_fee_per_sec,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            maintenance_status: MaintenanceStatus::None,
            withheld_rent: 0,
            inspector: None,
            repair_proof_hash: None,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: params.arbitrators,
        };

        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }



    pub fn get_lease_instance(env: Env, lease_id: u64) -> Result<LeaseInstance, LeaseError> {
        load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)
    }

    pub fn set_lease_instance_buyout_price(env: Env, lease_id: u64, landlord: Address, buyout_price: i128) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();
        lease.buyout_price = Some(buyout_price);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn pay_lease_instance_rent(env: Env, lease_id: u64, payment_amount: i128) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        require!(lease.active, "Lease is not active");
        Self::require_kyc(&env, &lease.landlord, &lease.tenant)?;
        Self::require_stablecoin(&env, &lease.payment_token)?;

        if lease.status == LeaseStatus::Pending {
            lease.status = LeaseStatus::Active;
        }

        if lease.maintenance_status == MaintenanceStatus::Reported || lease.maintenance_status == MaintenanceStatus::Fixed {
            lease.withheld_rent += payment_amount;
        } else {
            lease.cumulative_payments += payment_amount;
            lease.rent_paid += payment_amount;
        }
        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price && (lease.maintenance_status == MaintenanceStatus::None || lease.maintenance_status == MaintenanceStatus::Verified) {
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                if let (Some(nft), Some(id)) = (&lease.nft_contract, &lease.token_id) {
                    let client = nft_contract::NftClient::new(&env, nft);
                    client.transfer_from(&env.current_contract_address(), &env.current_contract_address(), &lease.tenant, id);
                }
                archive_lease(&env, lease_id, lease.clone(), env.current_contract_address());
                return Ok(());
            }
        }
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    /// Sets the pre-approved withdrawal address for the landlord.
    /// Only the landlord can call this function.
    pub fn set_withdrawal_address(
        env: Env,
        lease_id: u64,
        withdrawal_address: Address,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Authorize landlord
        lease.landlord.require_auth();

        lease.withdrawal_address = Some(withdrawal_address);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    /// Landlord withdraws accumulated rent to the pre-approved address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `lease_id` - Unique identifier of the lease
    /// * `token_contract_id` - The asset ID of the rent token to withdraw
    ///
    /// # Errors
    /// * `LeaseError::LeaseNotFound` - No lease exists for the given ID
    /// * `LeaseError::Unauthorised` - Caller is not the landlord
    /// * `LeaseError::WithdrawalAddressNotSet` - Withdrawal address has not been set
    ///
    /// # Panics
    /// Panics if there are no funds to withdraw.
    pub fn withdraw_rent(
        env: Env,
        lease_id: u64,
        token_contract_id: Address,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Authorize landlord
        lease.landlord.require_auth();

        if lease.payment_token != token_contract_id {
            return Err(LeaseError::PaymentTokenMismatch);
        }

        let withdrawal_address = lease
            .withdrawal_address
            .clone()
            .ok_or(LeaseError::WithdrawalAddressNotSet)?;

        let withdrawable_amount = lease.rent_paid - lease.rent_withdrawn;
        if withdrawable_amount <= 0 {
            panic!("No rent to withdraw");
        }

        // Transfer funds
        let token_client = soroban_sdk::token::Client::new(&env, &token_contract_id);
        token_client.transfer(
            &env.current_contract_address(),
            &withdrawal_address,
            &withdrawable_amount,
        );

        // Update state
        lease.rent_withdrawn += withdrawable_amount;
        save_lease_instance(&env, lease_id, &lease);

        Ok(())
    }

    /// Batch withdraw accumulated rent for multiple lease instances.
    ///
    /// Optimised to do a single token transfer for all leases to reduce gas.
    pub fn batch_withdraw_rent(
        env: Env,
        landlord: Address,
        lease_ids: soroban_sdk::Vec<u64>,
        token_contract_id: Address,
    ) -> Result<i128, LeaseError> {
        landlord.require_auth();

        let contract_address = env.current_contract_address();
        let token_client = soroban_sdk::token::Client::new(&env, &token_contract_id);

        let mut total_withdrawable: i128 = 0;
        let mut withdrawal_address: Option<Address> = None;

        for lease_id in lease_ids.iter() {
            let mut lease =
                load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

            if lease.landlord != landlord {
                return Err(LeaseError::Unauthorised);
            }

            if lease.payment_token != token_contract_id {
                return Err(LeaseError::PaymentTokenMismatch);
            }

            let lease_withdrawal_address = lease
                .withdrawal_address
                .clone()
                .ok_or(LeaseError::WithdrawalAddressNotSet)?;

            match &withdrawal_address {
                None => withdrawal_address = Some(lease_withdrawal_address),
                Some(expected) => {
                    if expected != &lease_withdrawal_address {
                        return Err(LeaseError::WithdrawalAddressMismatch);
                    }
                }
            }

            let withdrawable_amount = lease.rent_paid - lease.rent_withdrawn;
            if withdrawable_amount > 0 {
                total_withdrawable += withdrawable_amount;
                lease.rent_withdrawn += withdrawable_amount;
                save_lease_instance(&env, lease_id, &lease);
            }
        }

        if total_withdrawable <= 0 {
            panic!("No rent to withdraw");
        }

        token_client.transfer(
            &contract_address,
            &withdrawal_address.expect("withdrawal address not set"),
            &total_withdrawable,
        );

        Ok(total_withdrawable)
    }

    /// Terminates an expired lease and clears or archives its state from ledger storage.
    ///
    /// # Arguments
    /// * `env`      - The Soroban environment
    /// * `lease_id` - Unique identifier of the lease to terminate
    /// * `caller`   - Address of the party invoking termination (landlord, tenant, or admin)
    ///
    /// # Errors
    /// * `LeaseError::LeaseNotFound`    - No lease exists for the given ID
    /// * `LeaseError::LeaseNotExpired`  - Current ledger timestamp is before `end_date`
    /// * `LeaseError::RentOutstanding`  - One or more rent payments remain unpaid
    /// * `LeaseError::DepositNotSettled`- Security deposit has not been returned or claimed
    /// * `LeaseError::Unauthorised`     - Caller is not landlord, tenant, or admin
    ///
    /// # Security
    /// Caller must be the landlord, tenant, or an authorised protocol admin.
    /// Termination is idempotent: a second call on the same ID returns LeaseNotFound.
    /// Partial rent payment is never acceptable.
    /// The deposit must be fully Settled before termination is allowed.
    ///
    /// # Storage strategy
    /// DELETE — entry is fully removed from instance storage for maximum fee savings.
    /// TODO: consider archival strategy (archive_lease helper) if audit trail is required.
    pub fn terminate_lease(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<(), LeaseError> {
        // 1. Load lease — return LeaseNotFound if missing.
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // 2. Authorisation — caller must be landlord, tenant, or admin.
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
        if env.ledger().timestamp() < lease.end_date { return Err(LeaseError::LeaseNotExpired); }
        if lease.rent_paid_through < lease.end_date { return Err(LeaseError::RentOutstanding); }
        if lease.deposit_status == DepositStatus::Held || lease.deposit_status == DepositStatus::Disputed { return Err(LeaseError::DepositNotSettled); }

        let lease_duration = lease.end_date.saturating_sub(lease.start_date);
        let total_paid = lease.cumulative_payments;

        archive_lease(&env, lease_id, lease, caller);
        LeaseTerminated { lease_id }.publish(&env);
        LeaseEnded { id: lease_id, duration: lease_duration, total_paid }.publish(&env);
        Ok(())
    }

    pub fn reclaim_asset(env: Env, lease_id: u64, caller: Address, reason: String) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if caller != lease.landlord && caller != lease.tenant { return Err(LeaseError::Unauthorised); }
        caller.require_auth();
        AssetReclaimed { id: lease_id, reason }.publish(&env);
        Ok(())
    }

    pub fn conclude_lease(env: Env, lease_id: u64, landlord: Address, damage_deduction: i128) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if landlord != lease.landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();
        Self::require_kyc(&env, &lease.landlord, &lease.tenant)?;

        if env.ledger().timestamp() < lease.end_date { return Err(LeaseError::LeaseNotExpired); }
        if lease.rent_paid_through < lease.end_date { return Err(LeaseError::RentOutstanding); }

        if damage_deduction < 0 || damage_deduction > lease.security_deposit { return Err(LeaseError::InvalidDeduction); }

        let refund_amount = lease.security_deposit - damage_deduction;

        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        lease.active = false;
        save_lease_instance(&env, lease_id, &lease);
        Ok(refund_amount)
    }

    pub fn set_inspector(env: Env, lease_id: u64, landlord: Address, inspector: Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();
        lease.inspector = Some(inspector);
        save_lease_instance(&env, lease_id, &lease);
        Ok(())
    }

    pub fn report_maintenance_issue(env: Env, lease_id: u64, tenant: Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.tenant != tenant { return Err(LeaseError::Unauthorised); }
        tenant.require_auth();
        lease.maintenance_status = MaintenanceStatus::Reported;
        save_lease_instance(&env, lease_id, &lease);
        MaintenanceIssueReported { lease_id, tenant }.publish(&env);
        Ok(())
    }

    pub fn submit_repair_proof(env: Env, lease_id: u64, landlord: Address, proof_hash: BytesN<32>) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.landlord != landlord { return Err(LeaseError::Unauthorised); }
        landlord.require_auth();

        require!(lease.maintenance_status == MaintenanceStatus::Reported, "No issue reported");
        lease.repair_proof_hash = Some(proof_hash.clone());
        lease.maintenance_status = MaintenanceStatus::Fixed;
        save_lease_instance(&env, lease_id, &lease);
        RepairProofSubmitted { lease_id, landlord, proof_hash }.publish(&env);
        Ok(())
    }

    /// Reclaims an asset when the renter's payment stream runs dry (balance == 0).
    pub fn reclaim(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

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

        // Check renter_balance (deposit_amount == 0 implies stream is dry)
        if lease.deposit_amount > 0 {
            return Err(LeaseError::DepositNotSettled);
        }

        // If 0, transfer Asset NFT back to owner.
        if let (Some(nft_contract_addr), Some(token_id)) = (lease.nft_contract.clone(), lease.token_id) {
            delete_usage_rights(&env, nft_contract_addr.clone(), token_id);

            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }

        // Mark lease as Terminated.
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
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        match &lease.inspector { Some(expected) => if expected != &inspector { return Err(LeaseError::Unauthorised); }, None => return Err(LeaseError::Unauthorised), }
        inspector.require_auth();
        require!(lease.maintenance_status == MaintenanceStatus::Fixed, "Repair not marked as fixed");
        let released = lease.withheld_rent;
        lease.cumulative_payments += released;
        lease.rent_paid += released;
        lease.withheld_rent = 0;
        lease.maintenance_status = MaintenanceStatus::Verified;
        save_lease_instance(&env, lease_id, &lease);
        MaintenanceVerified { lease_id, inspector, withheld_released: released }.publish(&env);
        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address) -> Result<(), LeaseError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(LeaseError::Unauthorised); // Only set once if no admin exists
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn dispute_deposit(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if caller != lease.landlord && caller != lease.tenant { return Err(LeaseError::Unauthorised); }
        caller.require_auth();
        
        lease.deposit_status = DepositStatus::Disputed;
        lease.status = LeaseStatus::Disputed;
        save_lease_instance(&env, lease_id, &lease);
        
        DepositDisputed { lease_id, caller }.publish(&env);
        Ok(())
    }

    pub fn resolve_dispute(env: Env, lease_id: u64, landlord_bps: u32) -> Result<DepositReleasePartial, LeaseError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).ok_or(LeaseError::Unauthorised)?;
        admin.require_auth();
        
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if lease.deposit_status != DepositStatus::Disputed { return Err(LeaseError::DepositNotSettled); }

        let total = lease.deposit_amount;
        let (landlord_share, tenant_share) = leaseflow_math::calculate_deposit_split(total, landlord_bps)
            .ok_or(LeaseError::InvalidDeduction)?;

        lease.deposit_status = DepositStatus::Settled;
        lease.status = LeaseStatus::Terminated;
        save_lease_instance(&env, lease_id, &lease);

        let resolution = DepositReleasePartial { tenant_amount: tenant_share, landlord_amount: landlord_share };
        DisputeResolved { lease_id, resolution: resolution.clone() }.publish(&env);
        
        Ok(resolution)
    }

    /// Resolves a dispute by allowing a whitelisted arbitrator to conclude the lease.
    pub fn resolve_dispute_by_arbitrator(
        env: Env,
        lease_id: u64,
        arbitrator: Address,
        damage_deduction: i128,
    ) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Check if caller is a whitelisted arbitrator
        if !lease.arbitrators.contains(&arbitrator) {
            return Err(LeaseError::NotAnArbitrator);
        }
        
        // Require signature from the arbitrator
        arbitrator.require_auth();

        // Validate deduction against the security deposit
        if damage_deduction < 0 || damage_deduction > lease.security_deposit {
            return Err(LeaseError::InvalidDeduction);
        }

        let refund_amount = lease.security_deposit - damage_deduction;

        // Update state to terminate the lease and settle the deposit
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;

        lease.active = false;
        save_lease_instance(&env, lease_id, &lease);

        Ok(refund_amount)
    }

    /// Checks the tenant's payment status, updates the total debt,
    /// and triggers an EvictionEligible event if debt exceeds 2 months of rent.
    pub fn check_tenant_default(env: Env, lease_id: u64) -> Result<i128, LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
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
        }
        
        total_debt += lease.debt;

        // Assuming rent_amount represents 1 month of rent natively in the protocol
        let eviction_threshold = lease.rent_amount.saturating_mul(2);
        
        if total_debt >= eviction_threshold {
            EvictionEligible {
                lease_id,
                tenant: lease.tenant.clone(),
                debt: total_debt,
            }.publish(&env);
        }

        save_lease_instance(&env, lease_id, &lease);
        Ok(total_debt)
    }
}

mod test;
