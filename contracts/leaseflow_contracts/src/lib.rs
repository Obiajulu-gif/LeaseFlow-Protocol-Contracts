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
    pub rent_paid_through: u64,
    pub deposit_status: DepositStatus,
    pub buyout_price: Option<i128>,
    pub cumulative_payments: i128,
    pub maintenance_status: MaintenanceStatus,
    pub repair_proof_hash: Option<BytesN<32>>,
    pub withheld_rent: i128,
    pub inspector: Option<Address>,
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
    NftNotReturned = 8,
    UsageRightsNotFound = 9,
    UsageRightsExpired = 10,
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

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    // --- SIMPLE LEASE (Symbol-based) ---

    pub fn initialize_lease(env: Env, lease_id: Symbol, landlord: Address, tenant: Address, rent_amount: i128, deposit_amount: i128, duration: u64, property_uri: String) -> bool {
        landlord.require_auth();
        let start_date = env.ledger().timestamp();
        let end_date = start_date.saturating_add(duration);
        let lease = Lease {
            landlord, tenant, rent_per_sec: 0, late_fee_per_sec: 0, deposit_amount, start_date, end_date, property_uri, status: LeaseStatus::Pending, nft_contract: None, token_id: None, active: true, grace_period_end: end_date, late_fee_flat: 0, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time: end_date, buyout_price: None, cumulative_payments: 0,
        };
        env.storage().instance().set(&lease_id, &lease);
        true
    }

    pub fn create_lease(env: Env, landlord: Address, tenant: Address, _amount: i128) -> Symbol {
        landlord.require_auth();
        let lease_id = symbol_short!("lease");
        let lease = Lease {
            landlord, tenant, rent_per_sec: 0, late_fee_per_sec: 0, deposit_amount: 0, start_date: env.ledger().timestamp(), end_date: 0, property_uri: String::from_str(&env, ""), status: LeaseStatus::Pending, nft_contract: None, token_id: None, active: true, grace_period_end: 0, late_fee_flat: 0, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time: 0, buyout_price: None, cumulative_payments: 0,
        };
        env.storage().instance().set(&lease_id, &lease);
        lease_id
    }

    pub fn create_lease_with_nft(env: Env, lease_id: Symbol, landlord: Address, tenant: Address, rent_amount: i128, rent_rate_type: RateType, duration: u64, grace_period_end: u64, late_fee_flat: i128, late_fee_amount: i128, late_fee_rate_type: RateType, nft_contract_addr: Address, token_id: u128) -> Symbol {
        landlord.require_auth();
        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        nft_client.transfer_from(&env.current_contract_address(), &landlord, &env.current_contract_address(), &token_id);
        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);
        let lease = Lease {
            landlord, tenant: tenant.clone(), rent_per_sec: to_per_second(rent_amount, rent_rate_type), late_fee_per_sec: to_per_second(late_fee_amount, late_fee_rate_type), deposit_amount: 0, start_date: now, end_date: expiry_time, property_uri: String::from_str(&env, ""), status: LeaseStatus::Active, nft_contract: Some(nft_contract_addr.clone()), token_id: Some(token_id), active: true, grace_period_end, late_fee_flat, debt: 0, flat_fee_applied: false, seconds_late_charged: 0, rent_paid: 0, expiry_time, buyout_price: None, cumulative_payments: 0,
        };
        save_usage_rights(&env, nft_contract_addr, token_id, &UsageRights { renter: tenant, nft_contract: lease.nft_contract.clone().unwrap(), token_id, lease_id: lease_id.clone(), valid_until: expiry_time });
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
    }

    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.tenant == tenant, "Unauthorized");
        lease.status = LeaseStatus::Active;
        env.storage().instance().set(&lease_id, &lease);
        LeaseStarted { id: env.ledger().timestamp(), renter: tenant, rate: lease.rent_per_sec }.publish(&env);
        symbol_short!("active")
    }

    pub fn pay_rent(env: Env, lease_id: Symbol, payment_amount: i128) -> Symbol {
        let mut lease: Lease = env.storage().instance().get(&lease_id).expect("Lease not found");
        require!(lease.active, "Lease is not active");
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
        symbol_short!("paid")
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
        let lease = LeaseInstance { landlord, tenant: params.tenant, rent_amount: params.rent_amount, deposit_amount: params.deposit_amount, security_deposit: params.security_deposit, start_date: params.start_date, end_date: params.end_date, property_uri: params.property_uri, status: LeaseStatus::Pending, nft_contract: None, token_id: None, active: true, rent_paid: 0, rent_paid_through: params.start_date, deposit_status: DepositStatus::Held, buyout_price: None, cumulative_payments: 0, maintenance_status: MaintenanceStatus::None, repair_proof_hash: None, withheld_rent: 0, inspector: None, };
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
        if !lease.active { return Err(LeaseError::LeaseNotFound); }
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

    pub fn terminate_lease(env: Env, lease_id: u64, caller: Address) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        if caller != lease.landlord && caller != lease.tenant { return Err(LeaseError::Unauthorised); }
        caller.require_auth();
        if env.ledger().timestamp() < lease.end_date { return Err(LeaseError::LeaseNotExpired); }
        if lease.deposit_status == DepositStatus::Held || lease.deposit_status == DepositStatus::Disputed { return Err(LeaseError::DepositNotSettled); }
        archive_lease(&env, lease_id, lease, caller);
        LeaseTerminated { lease_id }.publish(&env);
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
        if damage_deduction < 0 || damage_deduction > lease.deposit_amount { return Err(LeaseError::InvalidDeduction); }
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = DepositStatus::Settled;
        save_lease_instance(&env, lease_id, &lease);
        Ok(lease.deposit_amount - damage_deduction)
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
    /// Reclaims an asset when the renter's payment stream runs dry (balance == 0).
    pub fn reclaim(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

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
        
        save_lease(&env, lease_id, &lease);

        AssetReclaimed {
            id: lease_id,
            reason: String::from_str(&env, "Payment stream ran dry"),
        }.publish(&env);

        Ok(())
    }

    /// Concludes a lease and processes security deposit refund with damage deductions.
    /// Only the landlord can call this function to approve the return and specify damage deductions.
    /// 
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `lease_id` - Unique identifier of the lease to conclude
    /// * `damage_deduction` - Amount to deduct from security deposit for damages
    /// 
    /// # Errors
    /// * `LeaseError::LeaseNotFound` - No lease exists for the given ID
    /// * `LeaseError::Unauthorised` - Caller is not the landlord
    /// * `LeaseError::LeaseNotExpired` - Lease has not yet expired
    /// * `LeaseError::RentOutstanding` - Rent has not been paid through end_date
    /// 
    /// # Returns
    /// Returns the refund amount (security_deposit - damage_deduction) to be returned to tenant
    pub fn conclude_lease(
        env: Env,
        lease_id: u64,
        landlord: Address,
        damage_deduction: i128,
    ) -> Result<i128, LeaseError> {
        // 1. Load lease
        let mut lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        // 2. Authorisation - only landlord can conclude lease
        if landlord != lease.landlord {
            return Err(LeaseError::Unauthorised);
        }
        landlord.require_auth();
        require!(lease.maintenance_status == MaintenanceStatus::Reported, "No issue reported");
        lease.repair_proof_hash = Some(proof_hash.clone());
        lease.maintenance_status = MaintenanceStatus::Fixed;
        save_lease_instance(&env, lease_id, &lease);
        RepairProofSubmitted { lease_id, landlord, proof_hash }.publish(&env);
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
}

mod test;
