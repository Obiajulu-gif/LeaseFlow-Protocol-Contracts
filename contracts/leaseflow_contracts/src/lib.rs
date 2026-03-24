#![no_std]
use soroban_sdk::{
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
}

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

/// Full lease record stored on-ledger.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseInstance {
    pub landlord: Address,
    pub tenant: Address,
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
    /// IPFS / HTTP URI pointing to the off-chain lease document.
    pub property_uri: String,
}

/// Archived record written to persistent storage on successful termination.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
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
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
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
        };
        save_lease(&env, lease_id, &lease);
        Ok(())
    }

    /// Returns a LeaseInstance by ID.
    pub fn get_lease_instance(env: Env, lease_id: u64) -> Result<LeaseInstance, LeaseError> {
        load_lease(&env, lease_id).ok_or(LeaseError::LeaseNotFound)
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
