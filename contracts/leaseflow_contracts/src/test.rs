#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, Event, String, Symbol,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, symbol_short};
use crate::{LeaseContract, LeaseContractClient, LeaseStatus};

#[test]
fn test_storage_management_and_ttl() {
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

    
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    
    let lease_id = symbol_short!("lease1");
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let rent_amount = 5000i128;
    let deposit_amount = 10000i128;
    let duration = 31_536_000u64; // 1 year
    let property_uri = String::from_str(&env, "ipfs://QmHash123");

    // ── 1. Create Lease: Core identities in Persistent storage ──────────────────
    client.initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &rent_amount,
        &deposit_amount,
        &duration,
        &property_uri,
    );

    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.rent_amount, rent_amount);
    assert_eq!(lease.status, LeaseStatus::Pending);

    client.activate_lease(&lease_id, &tenant);
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.status, LeaseStatus::Active);

    // ── 2. Pay Rent: Monthly receipts in Instance storage ──────────────────────
    let month = 1;
    let amount_paid = 5000i128;
    client.pay_rent(&lease_id, &month, &amount_paid);

    let receipt = client.get_receipt(&lease_id, &month);
    assert_eq!(receipt.lease_id, lease_id);
    assert_eq!(receipt.month, month);
    assert_eq!(receipt.amount, amount_paid);
    assert_eq!(receipt.date, 0); // Ledger starts at 0 

    // ── 3. TTL Extension Check (Simplified) ───────────────────────────────────
    client.extend_ttl(&lease_id);
}

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
    let expected_terminated = LeaseTerminated { lease_id: LEASE_ID };
    let expected_ended = LeaseEnded {
        id: LEASE_ID,
        duration: END - START,
        total_paid: 0, // From make_lease default
    };
    
    let events = env.events().all();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], expected_terminated.to_xdr(&env, &id));
    assert_eq!(events[1], expected_ended.to_xdr(&env, &id));
}

/// Tests that LeaseStarted event is emitted when a lease is activated.
#[test]
fn test_activate_lease_emits_started_event() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Create a pending lease first
    client.create_lease(&landlord, &tenant, &1000i128);

    // Act
    let result = client.activate_lease(&symbol_short!("lease"), &tenant);

    // Assert
    assert_eq!(result, symbol_short!("active"));
    
    // Check that LeaseStarted event was emitted
    // Use the expected timestamp as ID
    let expected_timestamp = env.ledger().timestamp();
    let expected = LeaseStarted {
        id: expected_timestamp,
        renter: tenant,
        rate: 0, // Will be 0 for simple lease
    };
    
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], expected.to_xdr(&env, &id));
}

/// Tests that AssetReclaimed event is emitted when an asset is reclaimed.
#[test]
fn test_reclaim_asset_emits_reclaimed_event() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let reason = String::from_str(&env, "Lease expired - asset returned");

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));

    // Act
    let result = client.reclaim_asset(&LEASE_ID, &landlord, &reason);

    // Assert
    assert_eq!(result, ());
    
    // Check that AssetReclaimed event was emitted
    let expected = AssetReclaimed {
        id: LEASE_ID,
        reason: reason.clone(),
    };
    
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], expected.to_xdr(&env, &id));
}

/// Tests that unauthorized reclaim_asset calls return error.
#[test]
fn test_reclaim_asset_unauthorized() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let reason = String::from_str(&env, "Unauthorized attempt");

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));

    // Act
    let result = client.reclaim_asset(&LEASE_ID, &unauthorized, &reason);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
    
    // No events should be emitted
    let events = env.events().all();
    assert_eq!(events.len(), 0);
}

// ---------------------------------------------------------------------------
// NFT Escrow Tests
// ---------------------------------------------------------------------------

/// Mock NFT contract for testing
#[contractclient(name = "MockNftClient")]
pub trait MockNftInterface {
    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    fn owner_of(env: Env, token_id: u128) -> Address;
}

/// Test that create_lease_with_nft transfers NFT to contract escrow
#[test]
fn test_create_lease_with_nft_escrows_to_contract() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 123;
    
    // Register mock NFT contract
    let nft_client = MockNftClient::new(&env, &nft_contract);
    
    // Create lease with NFT
    let lease_id = symbol_short!("test_lease");
    let result = client.create_lease_with_nft(
        &lease_id,
        &landlord,
        &tenant,
        &1000i128,
        &RateType::PerDay,
        &86400u64, // 1 day duration
        &2000u64,  // grace period
        &100i128,  // late fee flat
        &50i128,   // late fee amount
        &RateType::PerDay,
        &nft_contract,
        &token_id,
    );
    
    assert_eq!(result, symbol_short!("created"));
    
    // Verify usage rights were granted to tenant
    let usage_rights = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights.is_some());
    
    let rights = usage_rights.unwrap();
    assert_eq!(rights.renter, tenant);
    assert_eq!(rights.nft_contract, nft_contract);
    assert_eq!(rights.token_id, token_id);
    assert_eq!(rights.lease_id, lease_id);
}

/// Test that end_lease transfers NFT back to landlord and removes usage rights
#[test]
fn test_end_lease_returns_nft_to_landlord() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 456;
    
    // Create lease with NFT first
    let lease_id = symbol_short!("test_lease");
    client.create_lease_with_nft(
        &lease_id,
        &landlord,
        &tenant,
        &1000i128,
        &RateType::PerDay,
        &86400u64,
        &2000u64,
        &100i128,
        &50i128,
        &RateType::PerDay,
        &nft_contract,
        &token_id,
    );
    
    // Verify usage rights exist
    let usage_rights_before = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_before.is_some());
    
    // End lease as landlord
    let result = client.end_lease(&lease_id, &landlord);
    assert_eq!(result, symbol_short!("ended"));
    
    // Verify usage rights were removed
    let usage_rights_after = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_after.is_none());
    
    // Verify lease status is terminated
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.status, LeaseStatus::Terminated);
    assert!(!lease.active);
}

/// Test that unauthorized parties cannot end lease
#[test]
fn test_end_lease_unauthorized_fails() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let stranger = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 789;
    
    // Create lease with NFT
    let lease_id = symbol_short!("test_lease");
    client.create_lease_with_nft(
        &lease_id,
        &landlord,
        &tenant,
        &1000i128,
        &RateType::PerDay,
        &86400u64,
        &2000u64,
        &100i128,
        &50i128,
        &RateType::PerDay,
        &nft_contract,
        &token_id,
    );
    
    // Try to end lease as unauthorized party
    let result = client.try_end_lease(&lease_id, &stranger);
    assert!(result.is_err());
}

/// Test usage rights expiration
#[test]
fn test_usage_rights_expiration() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 999;
    
    // Create lease with NFT with short duration
    let lease_id = symbol_short!("test_lease");
    client.create_lease_with_nft(
        &lease_id,
        &landlord,
        &tenant,
        &1000i128,
        &RateType::PerDay,
        &100u64, // Very short duration
        &2000u64,
        &100i128,
        &50i128,
        &RateType::PerDay,
        &nft_contract,
        &token_id,
    );
    
    // Verify usage rights exist initially
    let usage_rights_before = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_before.is_some());
    
    // Advance time beyond lease duration
    env.ledger().with_mut(|l| l.timestamp += 200u64);
    
    // Verify usage rights have expired
    let usage_rights_after = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_after.is_none());
}

/// Test that tenant can also end lease
#[test]
fn test_end_lease_tenant_can_end() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 111;
    
    // Create lease with NFT
    let lease_id = symbol_short!("test_lease");
    client.create_lease_with_nft(
        &lease_id,
        &landlord,
        &tenant,
        &1000i128,
        &RateType::PerDay,
        &86400u64,
        &2000u64,
        &100i128,
        &50i128,
        &RateType::PerDay,
        &nft_contract,
        &token_id,
    );
    
    // End lease as tenant
    let result = client.end_lease(&lease_id, &tenant);
    assert_eq!(result, symbol_short!("ended"));
    
    // Verify usage rights were removed
    let usage_rights = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights.is_none());
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
#[should_panic(expected = "Lease not found")]
fn test_get_nonexistent_lease() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);
    client.get_lease(&symbol_short!("ghost"));
}

// ---------------------------------------------------------------------------
// Buyout functionality tests
// ---------------------------------------------------------------------------

#[test]
fn test_set_buyout_price_simple_lease() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &1000i128);
    
    // Set buyout price
    client.set_buyout_price(&symbol_short!("lease"), &landlord, &5000i128);
    
    let lease = client.get_lease();
    assert_eq!(lease.buyout_price, Some(5000i128));
    assert_eq!(lease.cumulative_payments, 0);
}

#[test]
fn test_set_buyout_price_unauthorized() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &1000i128);
    
    // Try to set buyout price as unauthorized user
    env.mock_all_auths();
    env.set_contract_auths(&[(&unauthorized, &symbol_short!("set_buyout_price"))]);
    
    let result = std::panic::catch_unwind(|| {
        client.set_buyout_price(&symbol_short!("lease"), &unauthorized, &5000i128);
    });
    
    assert!(result.is_err());
}

#[test]
fn test_buyout_with_simple_lease() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &1000i128);
    client.set_buyout_price(&symbol_short!("lease"), &landlord, &3000i128);
    
    // Make payments that reach the buyout price
    client.pay_rent(&symbol_short!("lease"), &1000i128);
    client.pay_rent(&symbol_short!("lease"), &1000i128);
    client.pay_rent(&symbol_short!("lease"), &1000i128);
    
    let lease = client.get_lease();
    assert_eq!(lease.cumulative_payments, 3000i128);
    assert!(!lease.active); // Should be inactive after buyout
}

#[test]
fn test_set_lease_instance_buyout_price() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    
    // Set buyout price
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &5000i128).unwrap();
    
    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.buyout_price, Some(5000i128));
    assert_eq!(lease.cumulative_payments, 0);
}

#[test]
fn test_lease_instance_buyout_execution() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &3000i128).unwrap();
    
    // Make payments that reach the buyout price
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128).unwrap();
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128).unwrap();
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128).unwrap();
    
    // Lease should be terminated and archived
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
    
    // Check historical record
    let record: HistoricalLease = env.as_contract(&id, || {
        env.storage()
            .persistent()
            .get(&DataKey::HistoricalLease(LEASE_ID))
            .expect("HistoricalLease not found")
    });
    
    assert_eq!(record.lease.cumulative_payments, 3000i128);
    assert_eq!(record.lease.status, LeaseStatus::Terminated);
    assert!(!record.lease.active);
}

#[test]
fn test_buyout_price_not_reached() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &5000i128).unwrap();
    
    // Make payments that don't reach the buyout price
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128).unwrap();
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128).unwrap();
    
    // Lease should still be active
    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.cumulative_payments, 2000i128);
    assert!(lease.active);
    assert_eq!(lease.status, LeaseStatus::Active);
}
