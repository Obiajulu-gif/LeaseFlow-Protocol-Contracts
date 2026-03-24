#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, Event, String,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LEASE_ID: u64 = 1;
const START: u64 = 1_000_000;
const END: u64 = 2_000_000;

fn make_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

fn make_lease(env: &Env, landlord: &Address, tenant: &Address) -> LeaseInstance {
    LeaseInstance {
        landlord: landlord.clone(),
        tenant: tenant.clone(),
        rent_amount: 1_000,
        deposit_amount: 2_000,
        start_date: START,
        end_date: END,
        rent_paid_through: END,                 // fully paid by default
        deposit_status: DepositStatus::Settled, // settled by default
        status: LeaseStatus::Active,
        property_uri: String::from_str(env, "ipfs://QmHash123"),
    }
}

/// Register the contract and return (contract_id, client).
fn setup(env: &Env) -> (Address, LeaseContractClient<'_>) {
    let id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(env, &id);
    (id, client)
}

/// Seed a LeaseInstance directly into contract storage (bypasses auth).
fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease(env, lease_id, lease));
}

/// Read a LeaseInstance directly from contract storage.
fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease(env, lease_id))
}

// ---------------------------------------------------------------------------
// Legacy test (preserved)
// ---------------------------------------------------------------------------

#[test]
fn test_lease() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &1000i128);
    let lease = client.get_lease();

    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, 1000);
    assert!(lease.active);
}

// ---------------------------------------------------------------------------
// terminate_lease tests
// ---------------------------------------------------------------------------

/// Happy path — expired, fully paid, settled lease is removed from storage.
#[test]
fn test_terminate_lease_success_deletes_storage() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, ());
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

/// Returns LeaseNotExpired when called before end_date.
#[test]
fn test_terminate_lease_before_end_date_fails() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END - 1); // still active

    // Act
    let result = client.try_terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotExpired)));
}

/// Returns RentOutstanding when rent has not been paid through end_date.
#[test]
fn test_terminate_lease_with_outstanding_rent_fails() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.rent_paid_through = END - 1; // one second short
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::RentOutstanding)));
}

/// Returns DepositNotSettled when deposit is still Held.
#[test]
fn test_terminate_lease_with_unsettled_deposit_fails() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
}

/// Returns DepositNotSettled when deposit is Disputed.
#[test]
fn test_terminate_lease_with_disputed_deposit_fails() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Disputed;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
}

/// Returns Unauthorised for a caller that is neither landlord, tenant, nor admin.
#[test]
fn test_terminate_lease_unauthorised_caller_fails() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let stranger = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_terminate_lease(&LEASE_ID, &stranger);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

/// Returns LeaseNotFound for a non-existent lease ID.
#[test]
fn test_terminate_lease_not_found_fails() {
    // Arrange
    let env = make_env();
    let (_, client) = setup(&env);
    let caller = Address::generate(&env);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act — no lease stored
    let result = client.try_terminate_lease(&99u64, &caller);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

/// Confirms the lease.terminated event is published on successful termination.
#[test]
fn test_terminate_lease_emits_terminated_event() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    client.terminate_lease(&LEASE_ID, &landlord);

    // Assert — the LeaseTerminated event must have been emitted.
    let expected = LeaseTerminated { lease_id: LEASE_ID };
    assert_eq!(
        env.events().all(),
        std::vec![expected.to_xdr(&env, &id)],
    );
}

/// Tenant can also invoke termination (not just landlord).
#[test]
fn test_terminate_lease_tenant_can_terminate() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.terminate_lease(&LEASE_ID, &tenant);

    // Assert
    assert_eq!(result, ());
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

/// Termination is idempotent — second call returns LeaseNotFound.
#[test]
fn test_terminate_lease_idempotent() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);
    client.terminate_lease(&LEASE_ID, &landlord);

    // Act — second call
    let result = client.try_terminate_lease(&LEASE_ID, &landlord);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

/// archive_lease helper moves the entry to persistent HistoricalLease storage.
#[test]
fn test_terminate_archived_lease_moves_to_historical() {
    // Arrange
    let env = make_env();
    let (id, _) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let lease = make_lease(&env, &landlord, &tenant);

    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act — call archive_lease inside the contract context
    env.as_contract(&id, || {
        save_lease(&env, LEASE_ID, &lease);
        archive_lease(&env, LEASE_ID, lease.clone(), landlord.clone());
    });

    // Assert — active storage cleared
    assert!(read_lease(&env, &id, LEASE_ID).is_none());

    // Assert — historical record exists in persistent storage
    let record: HistoricalLease = env.as_contract(&id, || {
        env.storage()
            .persistent()
            .get(&DataKey::HistoricalLease(LEASE_ID))
            .expect("HistoricalLease not found")
    });

    assert_eq!(record.lease, lease);
    assert_eq!(record.terminated_by, landlord);
    assert_eq!(record.terminated_at, END + 1);
}
