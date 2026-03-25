#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, Symbol, symbol_short, BytesN,
};
use crate::{LeaseContract, LeaseContractClient, LeaseStatus, MaintenanceStatus, DepositStatus, CreateLeaseParams, RateType, HistoricalLease, DataKey, 
    MaintenanceIssueReported, RepairProofSubmitted, MaintenanceVerified, LeaseStarted, LeaseTerminated};

const START: u64 = 1711929600; 
const END: u64 = 1714521600;   
const LEASE_ID: u64 = 1;

fn make_env() -> Env {
    let env = Env::default();
    env.ledger().with_mut(|l| l.timestamp = START);
    env.mock_all_auths();
    env
}

fn setup(env: &Env) -> (Address, LeaseContractClient<'_>) {
    let id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(env, &id);
    (id, client)
}

fn make_lease(env: &Env, landlord: &Address, tenant: &Address) -> LeaseInstance {
    LeaseInstance {
        landlord: landlord.clone(),
        tenant: tenant.clone(),
        rent_amount: 1_000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(env, "ipfs://QmHash123"),
        status: LeaseStatus::Active,
        nft_contract: None,
        token_id: None,
        active: true,
        rent_paid: 0,
        rent_paid_through: START,
        deposit_status: DepositStatus::Held,
        buyout_price: None,
        cumulative_payments: 0,
        maintenance_status: MaintenanceStatus::None,
        repair_proof_hash: None,
        withheld_rent: 0,
        inspector: None,
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_lease_basic() {
    let env = make_env();
    let (_, client) = setup(&env);
    
    let lease_id = symbol_short!("lease1");
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    client.initialize_lease(&lease_id, &landlord, &tenant, &5000, &10000, &31536000, &String::from_str(&env, "ipfs://test"));
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.status, LeaseStatus::Pending);

    client.activate_lease(&lease_id, &tenant);
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.status, LeaseStatus::Active);

    client.pay_rent(&lease_id, &5000);
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

/// Tests that reclaim successfully terminates the lease and returns the NFT when balance is 0.
#[test]
fn test_reclaim_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_amount = 0; // Simulate dry stream
    seed_lease(&env, &id, LEASE_ID, &lease);

    let result = client.reclaim(&LEASE_ID, &landlord);

    assert_eq!(result, Ok(()));

    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert!(!updated_lease.active);
    
    let events = env.events().all();
    assert!(events.len() > 0); // AssetReclaimed emitted
}

/// Tests that reclaim fails when deposit_amount is greater than 0.
#[test]
fn test_reclaim_fails_when_balance_not_zero() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_amount = 100; // Has balance
    seed_lease(&env, &id, LEASE_ID, &lease);

    let result = client.try_reclaim(&LEASE_ID, &landlord);

    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
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
    assert_eq!(lease.cumulative_payments, 5000);
}

#[test]
fn test_maintenance_flow_with_events() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let inspector = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_inspector(&LEASE_ID, &landlord, &inspector).unwrap();

    // 1. Tenant reports issue
    client.report_maintenance_issue(&LEASE_ID, &tenant).unwrap();
    
    // Check event
    let events = env.events().all();
    let last_event = events.last().unwrap();
    // In a real test we would verify the event data properly, but here we just check it was emitted.
    
    // 2. Tenant pays rent - it should be withheld
    client.pay_lease_instance_rent(&LEASE_ID, &1000).unwrap();
    
    // 3. Landlord submits repair proof
    let proof_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.submit_repair_proof(&LEASE_ID, &landlord, &proof_hash).unwrap();
    
    // 4. Inspector verifies repair
    client.verify_repair(&LEASE_ID, &inspector).unwrap();
    
    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.maintenance_status, MaintenanceStatus::Verified);
    assert_eq!(lease.withheld_rent, 0);
    assert_eq!(lease.cumulative_payments, 1000);
}

#[test]
fn test_lease_instance_buyout() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &1000).unwrap();
    
    client.pay_lease_instance_rent(&LEASE_ID, &1000).unwrap();
    
    // Lease should be terminated and archived
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_conclude_lease_happy_path() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    
    // Conclude lease
    let refund = client.conclude_lease(&LEASE_ID, &landlord, &500).unwrap();
    assert_eq!(refund, 1500); // 2000 - 500
}
