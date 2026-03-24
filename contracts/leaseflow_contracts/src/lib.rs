#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Symbol, BytesN, String,
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address, Env,
    String, Symbol,
};

// Re-export the pure math function so contract callers and tests can use it.
pub use leaseflow_math::calculate_total_cost;

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
}

// ── Rate helpers ──────────────────────────────────────────────────────────────

/// Describes the time unit in which a monetary rate is expressed by the caller.
/// All rates are normalised to **per-second** before being stored on-chain, so
/// internal arithmetic never needs to know the original unit.
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

/// Seconds of lease time granted per unit of funds added (1 day per unit).
pub const SECS_PER_UNIT: u64 = 86_400;

// ---------------------------------------------------------------------------
// LeaseInstance — full-featured lease used by terminate_lease and related fns
// ---------------------------------------------------------------------------

/// Deposit lifecycle: Held → Settled (returned or claimed) | Disputed
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositStatus {
    Held,
    Settled,
    Disputed,
}

/// Lease lifecycle status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Disputed,
    Terminated,
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};

    #[allow(dead_code)]
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

/// Seconds of lease time granted per unit of funds added (1 day per unit).
pub const SECS_PER_UNIT: u64 = 86_400;

pub mod proration;

/// Core lease record stored on-chain.
///
/// All rate fields (`rent_per_sec`, `late_fee_per_sec`) are normalised to
/// **per-second** by [`to_per_second`] at creation time — callers pass the
/// human-friendly value together with a [`RateType`] and conversion happens
/// once in the contract entry points.
/// Full lease record stored on-ledger.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseInstance {
    pub landlord: Address,
    pub tenant: Address,
    /// Per-second rent rate (normalised from the caller-supplied `RateType`).
    pub rent_per_sec: i128,
    /// Monthly / periodic rent amount in stroops.
    pub rent_amount: i128,
    /// Security deposit amount in stroops.
    pub deposit_amount: i128,
    /// Unix timestamp: lease start.
    pub start_date: u64,
    /// Unix timestamp: lease end — termination is only allowed after this.
    pub end_date: u64,
    /// Unix timestamp up to which rent has been paid. Must be >= end_date to terminate.
    pub rent_paid_through: u64,
    /// Deposit lifecycle state. Must be Settled before termination.
    pub deposit_status: DepositStatus,
    /// Lease lifecycle state.
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub grace_period_end: u64,
    /// One-time flat late fee applied the first second rent is overdue.
    pub late_fee_flat: i128,
    /// Per-second late fee (normalised from the caller-supplied `RateType`).
    pub late_fee_per_sec: i128,
    pub debt: i128,
    pub flat_fee_applied: bool,
    /// Total seconds of lateness for which the per-second fee has been charged.
    pub seconds_late_charged: u64,
    pub rent_paid: i128,
    pub expiry_time: u64,
    /// IPFS / HTTP URI pointing to the off-chain lease document.
    pub property_uri: String,
    /// Optional price at which the tenant can buy out the asset.
    pub buyout_price: Option<i128>,
    /// Total cumulative payments made by the tenant.
    pub cumulative_payments: i128,
}

/// Archived record written to persistent storage on successful termination.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseAmendment {
    /// Provide a new per-second rent rate, already normalised via
    /// [`to_per_second`] off-chain (or pass `None` to keep current value).
    pub new_rent_per_sec: Option<i128>,
    pub new_end_date: Option<u64>,
    pub landlord_signature: BytesN<32>,
    pub tenant_signature: BytesN<32>,
pub struct HistoricalLease {
    pub lease: LeaseInstance,
    /// Ledger timestamp at the moment of termination.
    pub terminated_at: u64,
    /// Address that invoked terminate_lease.
    pub terminated_by: Address,
}

/// Parameters for creating a new LeaseInstance. Grouped to keep entry-point arg count ≤ 7.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateLeaseParams {
    pub tenant: Address,
    pub rent_amount: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a lease is successfully terminated and removed from storage.
#[contractevent]
pub struct LeaseTerminated {
    pub lease_id: u64,
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

// ── Contract ──────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub enum DataKey {
    /// Simple legacy lease (symbol_short key kept for snapshot compatibility).
    SimpleLease,
    /// Active LeaseInstance keyed by numeric ID.
    Lease(u64),
    /// Historical record written after successful termination.
    HistoricalLease(u64),
    /// Protocol admin address.
    Admin,
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
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

/// Fetch a LeaseInstance from instance storage, or None.
pub fn load_lease(env: &Env, lease_id: u64) -> Option<LeaseInstance> {
    env.storage().instance().get(&DataKey::Lease(lease_id))
}

/// Persist a LeaseInstance to instance storage.
pub fn save_lease(env: &Env, lease_id: u64, lease: &LeaseInstance) {
    env.storage()
        .instance()
        .set(&DataKey::Lease(lease_id), lease);
}

/// Removes a LeaseInstance from active instance storage permanently.
///
/// Strategy: DELETE (preferred) — entry is fully removed, minimising ledger fees.
/// TODO: consider archival strategy if an on-chain audit trail is required.
pub fn delete_lease(env: &Env, lease_id: u64) {
    env.storage().instance().remove(&DataKey::Lease(lease_id));
}

/// Moves a LeaseInstance to the HistoricalLeases persistent map with a terminated_at timestamp.
/// Use this instead of delete_lease when an on-chain audit trail is required.
pub fn archive_lease(env: &Env, lease_id: u64, lease: LeaseInstance, terminated_by: Address) {
    let record = HistoricalLease {
        terminated_at: env.ledger().timestamp(),
        terminated_by,
        lease,
    };
    env.storage()
        .persistent()
        .set(&DataKey::HistoricalLease(lease_id), &record);
    // Remove from active storage after archiving.
    env.storage().instance().remove(&DataKey::Lease(lease_id));
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Calculates the prorated first month's rent based on the move-in timestamp.
    pub fn calculate_first_month_rent(_env: Env, start_date: u64, rent_amount: i128) -> i128 {
        proration::calculate_first_month_rent(start_date, rent_amount)
    }

    /// Initializes a lease with collateral lock (security deposit)
    pub fn initialize_lease(
        env: Env,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        deposit_amount: i128,
        start_date: u64,
        end_date: u64,
        property_uri: String,
    ) -> Symbol {
        let lease = Lease {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount,
            deposit_amount,
            start_date,
            end_date,
            property_uri: property_uri.clone(),
            status: LeaseStatus::Pending,
    /// Initializes a simple lease between a landlord and a tenant.
    pub fn create_lease(
        env: Env,
        landlord: Address,
        tenant: Address,
        amount: i128,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_per_day: i128,
    ) -> Symbol {
    /// Original function — unchanged behaviour, no NFT required.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
    /// Initializes a lease between a landlord and a tenant.
    /// `lease_id` uniquely identifies the lease in storage.
    /// `duration` sets the initial lease duration in seconds.
    
    pub fn create_lease(
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
    ) -> Symbol {
        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);

        let lease = Lease {
            landlord,
            tenant,
            //  normalise to per-second 
            rent_per_sec:      to_per_second(rent_amount,     rent_rate_type),
            late_fee_per_sec:  to_per_second(late_fee_amount, late_fee_rate_type),
            //
            deposit_amount: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
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

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("pending")
    }

    /// Creates a lease **and** immediately transfers an NFT from landlord to
    /// tenant.  Rate inputs follow the same `RateType` convention as
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
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &tenant,
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

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
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

        // ── Apply payment: clear debt first, then current-month rent ──────
        let mut remaining = payment_amount;

        if lease.debt > 0 {
            if remaining >= lease.debt {
                remaining -= lease.debt;
                lease.debt = 0;
            } else {
                lease.debt -= remaining;
                remaining = 0;
            }
    // -----------------------------------------------------------------------
    // Legacy simple-lease entry points (preserved)
    // -----------------------------------------------------------------------

    /// Initializes a simple lease between a landlord and a tenant.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
        let lease = Lease {
            landlord,
            tenant,
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

    // -----------------------------------------------------------------------
    // LeaseInstance entry points
    // -----------------------------------------------------------------------

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
    }

    /// Returns a LeaseInstance by ID.
    pub fn get_lease_instance(env: Env, lease_id: u64) -> Result<LeaseInstance, LeaseError> {
        load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)
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
    }

    /// Adds funds to an existing lease, extending `expiry_time` proportionally.
    /// Each unit of `amount` extends the lease by [`SECS_PER_UNIT`] seconds.
    /// Requires authorisation from the tenant.
    pub fn add_funds(env: Env, lease_id: Symbol, amount: i128) -> Symbol {
        assert!(amount > 0, "amount must be positive");

        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        lease.tenant.require_auth();

        let extra_secs = (amount as u64).saturating_mul(SECS_PER_UNIT);
        lease.expiry_time = lease.expiry_time.saturating_add(extra_secs);

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("extended")
        // 3. Expiry check — current time must be strictly after end_date.
        let now = env.ledger().timestamp();
        if now <= lease.end_date {
            return Err(LeaseError::LeaseNotExpired);
        }

        // 4. Rent check — rent must be paid through at least end_date.
        if lease.rent_paid_through < lease.end_date {
            return Err(LeaseError::RentOutstanding);
        }

        // 5. Deposit check — deposit must be fully settled.
        if lease.deposit_status != DepositStatus::Settled {
            return Err(LeaseError::DepositNotSettled);
        }

        // 6. State cleanup — delete from active storage.
        delete_lease(&env, lease_id);

        // 7. Emit termination event.
        LeaseTerminated { lease_id }.publish(&env);

        Ok(())
    }
}

mod test;
