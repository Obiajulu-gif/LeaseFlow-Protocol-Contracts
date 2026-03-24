#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, 
    Address, Env, String, Symbol, BytesN
};

// Re-export the pure math function so contract callers and tests can use it.
// pub use leaseflow_math::calculate_total_cost; // Only if available in dependencies

macro_rules! require {
    ($condition:expr, $error_msg:expr) => {
        if !$condition {
            panic!($error_msg);
        }
    };
}

// ── Rate helpers ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    PerSecond,
    PerHour,
    PerDay,
}

pub fn to_per_second(rate: i128, rate_type: RateType) -> i128 {
    match rate_type {
        RateType::PerSecond => rate,
        RateType::PerHour   => rate / 3_600,
        RateType::PerDay    => rate / 86_400,
    }
}

pub const SECS_PER_UNIT: u64 = 86_400;

// ── Status Enums ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositStatus {
    Held,
    Settled,
    Disputed,
}

/// Usage rights for NFT renters during lease period
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageRights {
    pub renter: Address,
    pub nft_contract: Address,
    pub token_id: u128,
    pub lease_id: Symbol,
    pub valid_until: u64,
}

/// Lease lifecycle status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
    Terminated,
}

// ── Structs ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseInstance {
    pub landlord: Address,
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub rent_paid: i128,
    pub debt: i128,
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
pub struct LeaseAmendment {
    pub new_rent_amount: Option<i128>,
    pub new_end_date: Option<u64>,
    pub landlord_signature: BytesN<32>,
    pub tenant_signature: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositReleasePartial {
    pub tenant_amount: i128,
    pub landlord_amount: i128,
}

#[contracttype]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

// ── Events ────────────────────────────────────────────────────────────────────

#[contractevent]
pub struct LeaseTerminated {
    pub lease_id: Symbol,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Lease(Symbol),
    Receipt(Symbol, u32),
    Admin,
    /// Usage rights for NFT renters.
    UsageRights(Address, u128),
}

// ---------------------------------------------------------------------------
// Error enum
// ---------------------------------------------------------------------------

/// All errors that can be returned by LeaseContract entry points.
///
/// # Security assumptions
/// - terminate_lease is callable by the landlord, tenant, or protocol admin only.
/// - Termination is idempotent: calling it on an already-deleted lease returns LeaseNotFound.
/// - Partial rent payment is never acceptable; rent_paid_through must reach end_date.
/// - The deposit must be fully Settled (returned to tenant OR claimed by landlord) before
///   termination is allowed. A Disputed deposit blocks termination.
#[contracterror]
#[derive(Debug, Clone, PartialEq)]
pub enum LeaseError {
    LeaseNotFound = 1,
    LeaseNotExpired = 2,
    RentOutstanding = 3,
    DepositNotSettled = 4,
    Unauthorised = 5,
    NftTransferFailed = 6,
    NftNotReturned = 7,
    UsageRightsNotFound = 8,
    UsageRightsExpired = 9,
}
// ── Storage Helpers ───────────────────────────────────────────────────────────

const DAY_IN_LEDGERS: u32 = 17280; // Assuming 5s ledger time
const MONTH_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 30;
const YEAR_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 365;

/// Fetch UsageRights from storage, or None.
pub fn load_usage_rights(env: &Env, nft_contract: Address, token_id: u128) -> Option<UsageRights> {
    env.storage().instance().get(&DataKey::UsageRights(nft_contract, token_id))
}

/// Save UsageRights to storage.
pub fn save_usage_rights(env: &Env, nft_contract: Address, token_id: u128, usage_rights: &UsageRights) {
    env.storage()
        .instance()
        .set(&DataKey::UsageRights(nft_contract, token_id), usage_rights);
}

/// Removes UsageRights from storage.
pub fn delete_usage_rights(env: &Env, nft_contract: Address, token_id: u128) {
    env.storage().instance().remove(&DataKey::UsageRights(nft_contract, token_id));
}

/// Fetch a LeaseInstance from instance storage, or None.
pub fn load_lease(env: &Env, lease_id: u64) -> Option<LeaseInstance> {
    env.storage().instance().get(&DataKey::Lease(lease_id))
pub fn load_lease(env: &Env, lease_id: &Symbol) -> Option<LeaseInstance> {
    env.storage().persistent().get(&DataKey::Lease(lease_id.clone()))
}

pub fn save_lease(env: &Env, lease_id: &Symbol, lease: &LeaseInstance) {
    let key = DataKey::Lease(lease_id.clone());
    env.storage().persistent().set(&key, lease);
    // identities stored in Persistent storage to survive ledger expirations
    env.storage().persistent().extend_ttl(&key, YEAR_IN_LEDGERS, YEAR_IN_LEDGERS);
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

// ── Contract Implementation ───────────────────────────────────────────────────

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Initializes a lease in Persistent storage.
    pub fn initialize_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        deposit_amount: i128,
        duration: u64,
        property_uri: String,
    ) -> bool {
        landlord.require_auth();

        let start_date = env.ledger().timestamp();
        let end_date = start_date.saturating_add(duration);

        let lease = LeaseInstance {
            landlord,
            tenant,
            rent_amount,
            deposit_amount,
            start_date,
            end_date,
            property_uri,
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time,
        };

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("pending")
    }

    /// Creates a lease **and** immediately transfers an NFT from landlord to
    /// contract escrow. Rate inputs follow the same `RateType` convention as
    /// [`create_lease`].
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
    ) -> Symbol {
        landlord.require_auth();

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        // Transfer NFT to contract escrow instead of directly to tenant
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
            tenant,
            rent_per_sec:      to_per_second(rent_amount,     rent_rate_type),
            late_fee_per_sec:  to_per_second(late_fee_amount, late_fee_rate_type),
            deposit_amount: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Active,
            nft_contract: Some(nft_contract_addr),
            token_id: Some(token_id),
            active: true,
            grace_period_end,
            late_fee_flat,
            debt: 0,
        };

        // Grant usage rights to the tenant for the lease duration
        let usage_rights = UsageRights {
            renter: tenant.clone(),
            nft_contract: nft_contract_addr,
            token_id,
            lease_id: lease_id.clone(),
            valid_until: expiry_time,
        };
        save_usage_rights(&env, nft_contract_addr, token_id, &usage_rights);

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
    }

    /// Ends a lease and returns the NFT from contract escrow to the landlord.
    /// Only the landlord or tenant can call this function.
    pub fn end_lease(env: Env, lease_id: Symbol, caller: Address) -> Symbol {
        let lease = Self::get_lease(env.clone(), lease_id.clone());
        
        // Authorization: only landlord or tenant can end the lease
        require!(
            lease.landlord == caller || lease.tenant == caller,
            "Unauthorized: Only landlord or tenant can end lease"
        );
        caller.require_auth();
        
        // Check if NFT is associated with this lease
        if let (Some(nft_contract_addr), Some(token_id)) = (lease.nft_contract, lease.token_id) {
            // Remove usage rights first
            delete_usage_rights(&env, nft_contract_addr, token_id);
            
            // Transfer NFT back to landlord from escrow
            let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
            nft_client.transfer_from(
                &env.current_contract_address(),
                &env.current_contract_address(),
                &lease.landlord,
                &token_id,
            );
        }
        
        // Update lease status to terminated
        let mut updated_lease = lease;
        updated_lease.status = LeaseStatus::Terminated;
        updated_lease.active = false;
        
        env.storage().instance().set(&lease_id, &updated_lease);
        symbol_short!("ended")
    }

    /// Activates a pending lease after the security deposit has been received.
    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.tenant == tenant, "Unauthorized: Only tenant can activate lease");
        require!(lease.status == LeaseStatus::Pending, "Lease is not in pending state");

        lease.status = LeaseStatus::Active;

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("active")
    }

    /// Updates the property metadata URI.
    pub fn update_property_uri(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        property_uri: String,
    ) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.landlord == landlord,
            "Unauthorized: Only landlord can update property URI"
        );
        lease.property_uri = property_uri;

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("updated")
    }

    /// Amends a lease with both landlord and tenant signatures.
    /// `amendment.new_rent_per_sec` should be pre-normalised by the caller
    /// using [`to_per_second`] if needed.
    pub fn amend_lease(env: Env, lease_id: Symbol, amendment: LeaseAmendment) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.status == LeaseStatus::Active, "Can only amend active leases");

        // Signatures are trusted here; a production implementation would
        // verify `amendment.landlord_signature` and `amendment.tenant_signature`.
        if let Some(new_rent) = amendment.new_rent_per_sec {
            lease.rent_per_sec = new_rent;
        }
        if let Some(new_end_date) = amendment.new_end_date {
            lease.end_date = new_end_date;
        }

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("amended")
    }

    /// Releases the security deposit according to `release_type`.
    pub fn release_deposit(
        env: Env,
        lease_id: Symbol,
        release_type: DepositRelease,
    ) -> Symbol {
        let lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.status == LeaseStatus::Active || lease.status == LeaseStatus::Expired,
            "Can only release deposit from active or expired leases"
        );

        match release_type {
            DepositRelease::FullRefund => symbol_short!("full_ref"),
            DepositRelease::PartialRefund(partial) => {
                require!(
                    partial.tenant_amount + partial.landlord_amount == lease.deposit_amount,
                    "Amounts must sum to total deposit"
                );
                symbol_short!("partial")
            }
            DepositRelease::Disputed => {
                let mut updated = lease;
                updated.status = LeaseStatus::Disputed;
                env.storage().instance().set(&lease_id, &updated);
                symbol_short!("disputed")
            }
        }
    }

    /// Checks if a given address has usage rights for a specific NFT.
    /// Returns the UsageRights if valid and not expired, None otherwise.
    pub fn check_usage_rights(env: Env, nft_contract: Address, token_id: u128, user: Address) -> Option<UsageRights> {
        if let Some(usage_rights) = load_usage_rights(&env, nft_contract, token_id) {
            let current_time = env.ledger().timestamp();
            
            // Check if the user is the renter and the rights haven't expired
            if usage_rights.renter == user && current_time <= usage_rights.valid_until {
                return Some(usage_rights);
            }
        }
        None
    }

    /// Returns the lease stored under `lease_id`.
    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found")
    }

    /// Processes a rent payment.
    ///
    /// Late fees are accrued in **per-second** terms using the stored
    /// `late_fee_per_sec` — no hardcoded 86 400 divisor is needed.
    /// The monthly rent threshold is derived from `rent_per_sec × 2_592_000`
    /// (30 × 86 400 seconds).
    pub fn pay_rent(env: Env, lease_id: Symbol, payment_amount: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        require!(lease.active, "Lease is not active");

        let current_time = env.ledger().timestamp();

        // ── Accrue late fees (all in per-second units) ────────────────────
        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;

            // One-time flat fee applied on the first overdue second.
            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            // Per-second fee: only charge newly elapsed seconds.
            if seconds_late > lease.seconds_late_charged {
                let newly_accrued = seconds_late - lease.seconds_late_charged;
                lease.debt += (newly_accrued as i128) * lease.late_fee_per_sec;
                lease.seconds_late_charged = seconds_late;
            }
        }
        save_lease(&env, &lease_id, &lease);
        true
    }

    /// Processes rent payment, saves receipt in Instance storage, and extends TTL.
    pub fn pay_rent(env: Env, lease_id: Symbol, month: u32, amount: i128) -> bool {
        let mut lease = load_lease(&env, &lease_id).expect("Lease not found");
        lease.tenant.require_auth();

        // Monthly payment receipts use Instance storage to keep costs low
        let receipt = Receipt {
            lease_id: lease_id.clone(),
            month,
            amount,
            date: env.ledger().timestamp(),
        };
        
        env.storage().instance().set(&DataKey::Receipt(lease_id.clone(), month), &receipt);

        lease.rent_paid += amount;
        save_lease(&env, &lease_id, &lease);

        // Keep the contract "alive" for the duration of the lease
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        true
    }

    pub fn get_lease(env: Env, lease_id: Symbol) -> LeaseInstance {
        load_lease(&env, &lease_id).expect("Lease not found")
    }

    pub fn get_receipt(env: Env, lease_id: Symbol, month: u32) -> Receipt {
        env.storage()
            .instance()
            .get(&DataKey::Receipt(lease_id, month))
            .expect("Receipt not found")
    }

    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> bool {
        let mut lease = load_lease(&env, &lease_id).expect("Lease not found");
        require!(lease.tenant == tenant, "Unauthorized");
        lease.status = LeaseStatus::Active;
        save_lease(&env, &lease_id, &lease);
        true
    }

    pub fn extend_ttl(env: Env, lease_id: Symbol) {
        let key = DataKey::Lease(lease_id);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(&key, MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        }
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
    }
}

mod test;
