#![no_std]
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, 
    Address, Env, String, Symbol, BytesN
};

// Re-export the pure math function so contract callers and tests can use it.
// pub use leaseflow_math::calculate_total_cost; // Only if available in dependencies

// ---------------------------------------------------------------------------
// Existing simple Lease struct (preserved for backwards compatibility)
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub amount: i128,
    pub active: bool,
    /// Optional price at which the tenant can buy out the asset.
    pub buyout_price: Option<i128>,
    /// Total cumulative payments made by the tenant.
    pub cumulative_payments: i128,
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
    pub expiry_time: u64,
    /// IPFS / HTTP URI pointing to the off-chain lease document.
    pub property_uri: String,
    /// Optional price at which the tenant can buy out the asset.
    pub buyout_price: Option<i128>,
    /// Total cumulative payments made by the tenant.
    pub cumulative_payments: i128,
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

/// Emitted when a lease starts and the asset becomes available to the renter.
#[contractevent]
pub struct LeaseStarted {
    pub id: u64,
    pub renter: Address,
    pub rate: i128,
}

/// Emitted when a lease ends and total payment information is available.
#[contractevent]
pub struct LeaseEnded {
    pub id: u64,
    pub duration: u64,
    pub total_paid: i128,
}

/// Emitted when an asset is reclaimed by the landlord or system.
#[contractevent]
pub struct AssetReclaimed {
    pub id: u64,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------
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
            buyout_price: None,
            cumulative_payments: 0,
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
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time,
            buyout_price: None,
            cumulative_payments: 0,
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
        
        // Emit LeaseStarted event for frontend notification
        // Use the current timestamp as a simple ID for the event
        let event_id = env.ledger().timestamp();
        LeaseStarted {
            id: event_id,
            renter: tenant,
            rate: lease.rent_per_sec,
        }.publish(&env);
        
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
            active: true,
            buyout_price: None,
            cumulative_payments: 0,
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
    }

    /// Sets the buyout price for a lease. Can only be called by the landlord.
    pub fn set_buyout_price(env: Env, lease_id: Symbol, landlord: Address, buyout_price: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        
        require!(
            lease.landlord == landlord,
            "Unauthorized: Only landlord can set buyout price"
        );
        require!(buyout_price > 0, "Buyout price must be positive");
        
        lease.buyout_price = Some(buyout_price);
        
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("buyout_set")
    }

    /// Returns the current simple lease details stored in the contract.
    pub fn get_lease(env: Env) -> Lease {
        env.storage()
            .instance()
            .get(&symbol_short!("lease"))
            .expect("Lease not found")
    }
            date: env.ledger().timestamp(),
        };
        
        env.storage().instance().set(&DataKey::Receipt(lease_id.clone(), month), &receipt);

        lease.rent_paid += amount;
        save_lease(&env, &lease_id, &lease);

    /// Creates a full LeaseInstance keyed by lease_id.
    pub fn create_lease_instance(
        env: Env,
        lease_id: u64,
        landlord: Address,
        params: CreateLeaseParams,
    ) -> Result<(), LeaseError> {
        landlord.require_auth();
        let lease = LeaseInstance {
            landlord,
            tenant: params.tenant,
            rent_amount: params.rent_amount,
            deposit_amount: params.deposit_amount,
            start_date: params.start_date,
            end_date: params.end_date,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            status: LeaseStatus::Pending,
            property_uri: params.property_uri,
            rent_per_sec: 0,
            nft_contract: None,
            token_id: None,
            active: true,
            grace_period_end: 0,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time: 0,
            buyout_price: None,
            cumulative_payments: 0,
        };
        save_lease(&env, lease_id, &lease);
        Ok(())
        // Keep the contract "alive" for the duration of the lease
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
        
        true
    }

    pub fn get_lease(env: Env, lease_id: Symbol) -> LeaseInstance {
        load_lease(&env, &lease_id).expect("Lease not found")
    }

    /// Sets the buyout price for a LeaseInstance. Can only be called by the landlord.
    pub fn set_lease_instance_buyout_price(
        env: Env,
        lease_id: u64,
        landlord: Address,
        buyout_price: i128,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if lease.landlord != landlord {
            return Err(LeaseError::Unauthorised);
        }
        if buyout_price <= 0 {
            panic!("Buyout price must be positive");
        }
        
        lease.buyout_price = Some(buyout_price);
        save_lease(&env, lease_id, &lease);
        Ok(())
    }

    /// Processes a rent payment for LeaseInstance and checks for buyout condition.
    pub fn pay_lease_instance_rent(
        env: Env,
        lease_id: u64,
        payment_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;
        
        if !lease.active {
            return Err(LeaseError::LeaseNotFound);
        }
        
        // Track cumulative payments
        lease.cumulative_payments += payment_amount;
        
        // Check for buyout condition
        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                // Transfer ownership to tenant
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                
                // If there's an NFT, transfer it to the tenant
                if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id) {
                    let nft_client = nft_contract::NftClient::new(&env, nft_contract);
                    nft_client.transfer_from(
                        &env.current_contract_address(),
                        &lease.landlord,
                        &lease.tenant,
                        token_id,
                    );
                }
                
                // Archive the lease after buyout
                archive_lease(&env, lease_id, lease, env.current_contract_address());
                return Ok(());
            }
        }
        
        save_lease(&env, lease_id, &lease);
        Ok(())
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
        let lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

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

        if remaining > 0 {
            lease.rent_paid += remaining;
            lease.cumulative_payments += payment_amount;

            // Monthly rent = per-second rate × seconds-in-30-days.
            let monthly_rent = lease.rent_per_sec.saturating_mul(2_592_000);
            if lease.rent_paid >= monthly_rent {
                lease.rent_paid -= monthly_rent;
                lease.grace_period_end = lease.grace_period_end.saturating_add(2_592_000);
                lease.flat_fee_applied = false;
                lease.seconds_late_charged = 0;
            }
        }

        env.storage().instance().set(&lease_id, &lease);
        
        // Check for buyout condition
        if let Some(buyout_price) = lease.buyout_price {
            if lease.cumulative_payments >= buyout_price {
                // Transfer ownership to tenant
                lease.active = false;
                lease.status = LeaseStatus::Terminated;
                
                // If there's an NFT, transfer it to the tenant
                if let (Some(nft_contract), Some(token_id)) = (&lease.nft_contract, &lease.token_id) {
                    let nft_client = nft_contract::NftClient::new(&env, nft_contract);
                    nft_client.transfer_from(
                        &env.current_contract_address(),
                        &lease.landlord,
                        &lease.tenant,
                        token_id,
                    );
                }
            }
        }
        
        symbol_short!("paid")
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

        // 6. State cleanup — delete from active storage.
        let lease_duration = lease.end_date.saturating_sub(lease.start_date);
        let total_payments = lease.rent_paid;

        delete_lease(&env, lease_id);

        // 7. Emit termination event.
        LeaseTerminated { lease_id }.publish(&env);
        
        // 8. Emit LeaseEnded event for frontend notification
        LeaseEnded {
            id: lease_id,
            duration: lease_duration,
            total_paid: total_payments,
        }.publish(&env);

        Ok(())
    }

    /// Reclaims an asset from a lease, typically called by landlord or system.
    /// Emits AssetReclaimed event for frontend notification.
    pub fn reclaim_asset(
        env: Env,
        lease_id: u64,
        caller: Address,
        reason: String,
    ) -> Result<(), LeaseError> {
        // Load lease
        let lease = load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

        // Authorisation — caller must be landlord, tenant, or admin.
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

        // Emit AssetReclaimed event for frontend notification
        AssetReclaimed {
            id: lease_id,
            reason: reason.clone(),
        }.publish(&env);

        Ok(())
        env.storage().instance().extend_ttl(MONTH_IN_LEDGERS, YEAR_IN_LEDGERS);
    }
}

mod test;
