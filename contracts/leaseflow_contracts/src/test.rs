#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, contract, contractimpl, contractclient, Event,
};
use soroban_sdk::Event;
use crate::{LeaseContract, LeaseContractClient, LeaseStatus, MaintenanceStatus, DepositStatus, CreateLeaseParams, RateType, HistoricalLease, DataKey, 
    MaintenanceIssueReported, RepairProofSubmitted, MaintenanceVerified, LeaseStarted, LeaseTerminated, DepositReleasePartial, EvictionEligible, EmergencyRentPaused, EmergencyRentResumed};

const START: u64 = 1711929600; 
const END: u64 = 1714521600;   
const LEASE_ID: u64 = 1;

// --- KYC Mock ---
#[contract]
pub struct KycMock;

#[contractimpl]
impl KycMock {
    pub fn is_verified(env: Env, address: Address) -> bool {
        env.storage().instance().get(&address).unwrap_or(false)
    }
    pub fn set_verified(env: Env, address: Address, status: bool) {
        env.storage().instance().set(&address, &status);
    }
}

// --- NFT Mock ---
#[contract]
pub struct MockNft;

#[contractimpl]
impl MockNft {
    pub fn transfer_from(env: Env, _spender: Address, _from: Address, to: Address, token_id: u128) {
        env.storage().instance().set(&token_id, &to);
    }

    pub fn owner_of(env: Env, token_id: u128) -> Address {
        env.storage().instance().get(&token_id).unwrap()
    }
}

// --- Token Mock (minimal subset used by withdraw tests) ---
#[contract]
pub struct TokenMock;

#[contractimpl]
impl TokenMock {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let current: i128 = env.storage().instance().get(&to).unwrap_or(0);
        env.storage().instance().set(&to, &(current + amount));
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().instance().get(&id).unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let from_balance: i128 = env.storage().instance().get(&from).unwrap_or(0);
        require!(from_balance >= amount, "insufficient balance");
        env.storage().instance().set(&from, &(from_balance - amount));

        let to_balance: i128 = env.storage().instance().get(&to).unwrap_or(0);
        env.storage().instance().set(&to, &(to_balance + amount));
    }
}

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
        payment_token: Address::generate(env),
        active: true,
        rent_paid: 0,
        expiry_time: END,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: 0,
        deposit_status: DepositStatus::Held,
        rent_per_sec: 0,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        flat_fee_applied: false,
        seconds_late_charged: 0,
        withdrawal_address: None,
        rent_withdrawn: 0,
        arbitrators: soroban_sdk::Vec::new(env),
        // Emergency pause fields
        paused: false,
        pause_reason: None,
        paused_at: None,
        pause_initiator: None,
        total_paused_duration: 0,
        // Other missing fields
        payment_token: Address::generate(env),
        maintenance_status: MaintenanceStatus::None,
        withheld_rent: 0,
        inspector: None,
        repair_proof_hash: None,
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_stablecoin_enforcement() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);
    let usdc = Address::generate(&env);
    let volatile_token = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &usdc);

    let lease_id = symbol_short!("lease1");
    let uri = String::from_str(&env, "ipfs://test");

    // 1. Should fail with volatile token
    let res = client.try_initialize_lease(&lease_id, &landlord, &tenant, &5000, &10000, &31536000, &uri, &volatile_token);
    assert!(res.is_err());

    // 2. Should succeed with USDC
    client.initialize_lease(&lease_id, &landlord, &tenant, &5000, &10000, &31536000, &uri, &usdc);
    let lease = client.get_lease(&lease_id);
    assert_eq!(lease.payment_token, usdc);
}

#[test]
fn test_lease_basic() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);
    
    let lease_id = symbol_short!("lease1");
    client.initialize_lease(&lease_id, &landlord, &tenant, &5000, &10000, &31536000, &String::from_str(&env, "ipfs://test"), &token);
    
    client.activate_lease(&lease_id, &tenant);
    client.pay_rent(&lease_id, &5000);
    
    // ── 2. Pay Rent: Monthly receipts in Instance storage ──────────────────────
    let month = 1;
    let amount_paid = 5000i128;
    client.pay_rent_receipt(&lease_id, &month, &amount_paid);

    let receipt = client.get_receipt(&lease_id, &month);
    assert_eq!(receipt.lease_id, lease_id);
    assert_eq!(receipt.month, month);
    assert_eq!(receipt.amount, amount_paid);
    assert_eq!(receipt.date, START);

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
    let events = events.events();
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
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    // Create a pending lease first
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let lease_id = symbol_short!("lease");
    let uri = String::from_str(&env, "ipfs://test");
    client.initialize_lease(&lease_id, &landlord, &tenant, &1000i128, &0i128, &60u64, &uri, &token);

    // Act
    let result = client.activate_lease(&lease_id, &tenant);

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
    let events = events.events();
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
    let events = events.events();
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
    let result = client.try_reclaim_asset(&LEASE_ID, &unauthorized, &reason);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
    
    // No events should be emitted
    let events = env.events().all();
    let events = events.events();
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

    client.reclaim(&LEASE_ID, &landlord);

    let events = env.events().all();
    let events = events.events();
    assert!(events.len() > 0); // AssetReclaimed emitted

    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert!(!updated_lease.active);
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

/// Test that create_lease_with_nft transfers NFT to contract escrow
#[test]
fn test_create_lease_with_nft_escrows_to_contract() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = env.register(MockNft, ());
    let token_id: u128 = 123;
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);
    
    let nft_client = MockNftClient::new(&env, &nft_contract);
    
    // Create lease with NFT
    let lease_id = symbol_short!("tst_lease");
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
        &token,
    );
    
    assert_eq!(result, symbol_short!("created"));

    // Verify NFT is escrowed to the lease contract
    assert_eq!(nft_client.owner_of(&token_id), contract_id);
    
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
    let nft_contract = env.register(MockNft, ());
    let token_id: u128 = 456;
    let admin = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let nft_client = MockNftClient::new(&env, &nft_contract);
    
    // Create lease with NFT first
    let lease_id = symbol_short!("tst_lease");
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
        &token,
    );

    // Escrowed to contract
    assert_eq!(nft_client.owner_of(&token_id), contract_id);
    
    // Verify usage rights exist
    let usage_rights_before = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_before.is_some());
    
    // End lease as landlord
    let result = client.end_lease(&lease_id, &landlord);
    assert_eq!(result, symbol_short!("ended"));
    
    // Verify usage rights were removed
    let usage_rights_after = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_after.is_none());
    
    // Verify NFT returned to landlord
    assert_eq!(nft_client.owner_of(&token_id), landlord);
}

#[test]
fn test_maintenance_flow_with_events() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let inspector = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_inspector(&LEASE_ID, &landlord, &inspector);
    client.report_maintenance_issue(&LEASE_ID, &tenant);
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.withheld_rent, 1000);
}

#[test]
fn test_batch_withdraw_rent_aggregates_payout() {
    let env = make_env();
    let (lease_contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant_1 = Address::generate(&env);
    let tenant_2 = Address::generate(&env);
    let withdrawal = Address::generate(&env);
    let admin = Address::generate(&env);
    let token_contract_id = env.register(TokenMock, ());

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token_contract_id);

    let lease_id_1: u64 = 1;
    let lease_id_2: u64 = 2;

    let params_1 = CreateLeaseParams {
        tenant: tenant_1,
        rent_amount: 1000,
        deposit_amount: 0,
        security_deposit: 0,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token_contract_id.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    let params_2 = CreateLeaseParams {
        tenant: tenant_2,
        rent_amount: 1000,
        deposit_amount: 0,
        security_deposit: 0,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token_contract_id.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    client.create_lease_instance(&lease_id_1, &landlord, &params_1);
    client.create_lease_instance(&lease_id_2, &landlord, &params_2);
    client.set_withdrawal_address(&lease_id_1, &withdrawal);
    client.set_withdrawal_address(&lease_id_2, &withdrawal);

    // Record rent owed.
    client.pay_lease_instance_rent(&lease_id_1, &100i128);
    client.pay_lease_instance_rent(&lease_id_2, &200i128);

    // Fund the lease contract with enough tokens to pay out.
    let token_client = TokenMockClient::new(&env, &token_contract_id);
    token_client.mint(&lease_contract_id, &300i128);

    let mut lease_ids = soroban_sdk::Vec::new(&env);
    lease_ids.push_back(lease_id_1);
    lease_ids.push_back(lease_id_2);

    let withdrawn = client.batch_withdraw_rent(&landlord, &lease_ids, &token_contract_id);
    assert_eq!(withdrawn, 300i128);

    assert_eq!(token_client.balance(&withdrawal), 300i128);
    assert_eq!(token_client.balance(&lease_contract_id), 0i128);

    let lease_1 = client.get_lease_instance(&lease_id_1);
    let lease_2 = client.get_lease_instance(&lease_id_2);
    assert_eq!(lease_1.rent_withdrawn, 100i128);
    assert_eq!(lease_2.rent_withdrawn, 200i128);
}

#[test]
fn test_lease_instance_buyout() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &3000i128);
    
    // Make payments that reach the buyout price
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128);
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128);
    client.pay_lease_instance_rent(&LEASE_ID, &1000i128);
    
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
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &2000);
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
    // Lease should still be active
    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.cumulative_payments, 1000i128);
    assert!(lease.active);
    assert_eq!(lease.status, LeaseStatus::Active);
}

// ---------------------------------------------------------------------------
// conclude_lease tests
// ---------------------------------------------------------------------------

/// Happy path - landlord concludes lease with no damage deductions, full refund
#[test]
fn test_conclude_lease_no_damages_full_refund() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held; // Reset to Held for conclusion
    lease.status = LeaseStatus::Active; // Reset to Active
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.conclude_lease(&LEASE_ID, &landlord, &0i128);

    // Assert
    assert_eq!(result, 500); // Full security deposit refunded
    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert_eq!(updated_lease.deposit_status, DepositStatus::Settled);
}

/// Happy path - landlord concludes lease with damage deductions, partial refund
#[test]
fn test_conclude_lease_with_damages_partial_refund() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held; // Reset to Held for conclusion
    lease.status = LeaseStatus::Active; // Reset to Active
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.conclude_lease(&LEASE_ID, &landlord, &200i128);

    // Assert
    assert_eq!(result, 300); // 500 - 200 = 300 refunded
    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert_eq!(updated_lease.deposit_status, DepositStatus::Settled);
}

/// Returns Unauthorised when tenant tries to conclude lease
#[test]
fn test_conclude_lease_tenant_unauthorised() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_conclude_lease(&LEASE_ID, &tenant, &100i128);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

/// Returns LeaseNotExpired when concluding before end_date
#[test]
fn test_conclude_lease_before_end_date() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END - 1); // Before end date

    // Act
    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &0i128);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotExpired)));
}

/// Returns RentOutstanding when rent is not fully paid
#[test]
fn test_conclude_lease_with_outstanding_rent() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    lease.rent_paid_through = END - 1; // Rent not fully paid
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &0i128);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::RentOutstanding)));
}

/// Returns InvalidDeduction when damage deduction is negative
#[test]
fn test_conclude_lease_negative_deduction() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &-100i128);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::InvalidDeduction)));
}

/// Returns InvalidDeduction when damage deduction exceeds security deposit
#[test]
fn test_conclude_lease_excessive_deduction() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &600i128); // More than 500 deposit

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::InvalidDeduction)));
}

/// Returns LeaseNotFound for non-existent lease
#[test]
fn test_conclude_lease_not_found() {
    // Arrange
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Act
    let result = client.try_conclude_lease(&99u64, &landlord, &0i128);

    // Assert
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

/// Test create_lease_instance with security_deposit
#[test]
fn test_create_lease_instance_with_security_deposit() {
    // Arrange
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
    };

    // Act
    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Assert
    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.security_deposit, 500);
    assert_eq!(lease.status, LeaseStatus::Pending);
    assert_eq!(lease.deposit_status, DepositStatus::Held);
}

#[test]
fn test_tenant_default_scenario_3_months_non_payment() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);
    
    // Set 1 month = 30 days = 2,592,000 seconds
    let month_in_secs: u64 = 2_592_000;
    let rent_per_sec = 1i128;
    let rent_amount = (month_in_secs as i128) * rent_per_sec; 
    let grace_period_secs = 5 * 86400; // 5 days
    
    let start_date = 10_000_000u64;
    env.ledger().with_mut(|l| l.timestamp = start_date);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount,
        deposit_amount: rent_amount * 2,
        security_deposit: rent_amount,
        start_date,
        end_date: start_date + month_in_secs * 12,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec,
        grace_period_end: start_date + month_in_secs + grace_period_secs,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    
    // Fast forward 1 month and 4 days (within grace period of first month)
    env.ledger().with_mut(|l| l.timestamp = start_date + month_in_secs + grace_period_secs - 1);
    let debt_1 = client.check_tenant_default(&LEASE_ID);
    // debt should be unpaid rent for ~1 month (no late fees since still in grace period)
    assert_eq!(debt_1, (month_in_secs + grace_period_secs - 1) as i128 * rent_per_sec);
    
    // Fast forward 1 month and 6 days (grace period exceeded)
    env.ledger().with_mut(|l| l.timestamp = start_date + month_in_secs + grace_period_secs + 1);
    let debt_2 = client.check_tenant_default(&LEASE_ID);
    // Debt should include flat fee (100) + 1 second of late fee (2) + unpaid rent
    let expected_unpaid_2 = (month_in_secs + grace_period_secs + 1) as i128 * rent_per_sec;
    assert_eq!(debt_2, expected_unpaid_2 + 100 + 2);
    
    // Fast forward 3 months
    let three_months = start_date + month_in_secs * 3;
    env.ledger().with_mut(|l| l.timestamp = three_months);
    let debt_3 = client.check_tenant_default(&LEASE_ID);
    
    // Unpaid rent = 3 months
    let expected_unpaid_3 = (month_in_secs * 3) as i128 * rent_per_sec;
    let late_seconds = three_months - (start_date + month_in_secs + grace_period_secs);
    let expected_late_fees = 100 + (late_seconds as i128 * 2);
    assert_eq!(debt_3, expected_unpaid_3 + expected_late_fees);
    
    // Threshold is 2 * rent_amount. Eviction event should be emitted.
    let events = env.events().all();
    let events = events.events();
    assert!(events.len() > 0);
    
    let expected_event = EvictionEligible {
        lease_id: LEASE_ID,
        tenant: tenant.clone(),
        debt: debt_3,
    };
    let last_index = events.len() - 1;
    assert_eq!(events[last_index], expected_event.to_xdr(&env, &id));
}

// ── Emergency Rent Pause Tests ────────────────────────────────────────────────

#[test]
fn test_emergency_pause_rent_by_admin() {
    let env = make_env();
    let (id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Test admin can pause rent
    let reason = String::from_str(&env, "Flood damage - property uninhabitable");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Verify lease is paused
    let (paused, pause_reason, paused_at, _) = client.get_pause_status(&LEASE_ID).unwrap();
    assert!(paused);
    assert_eq!(pause_reason.unwrap(), reason);
    assert_eq!(paused_at.unwrap(), START);

    // Verify lease status changed to Paused
    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.status, LeaseStatus::Paused);

    // Verify event was emitted
    let events = env.events().all();
    assert!(events.len() > 0);
}

#[test]
fn test_emergency_pause_rent_by_landlord() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Test landlord can pause rent
    let reason = String::from_str(&env, "Earthquake - building condemned");
    client.emergency_pause_rent(&LEASE_ID, &landlord, &reason);

    // Verify lease is paused
    let (paused, pause_reason, _, _) = client.get_pause_status(&LEASE_ID).unwrap();
    assert!(paused);
    assert_eq!(pause_reason.unwrap(), reason);
}

#[test]
fn test_emergency_pause_rent_by_arbitrator() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let arbitrator = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let mut arbitrators = soroban_sdk::Vec::new(&env);
    arbitrators.push_back(arbitrator.clone());

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Test arbitrator can pause rent
    let reason = String::from_str(&env, "Hurricane - mandatory evacuation");
    client.emergency_pause_rent(&LEASE_ID, &arbitrator, &reason);

    // Verify lease is paused
    let (paused, _, _, _) = client.get_pause_status(&LEASE_ID).unwrap();
    assert!(paused);
}

#[test]
fn test_emergency_pause_rent_unauthorized() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Test unauthorized user cannot pause rent
    let reason = String::from_str(&env, "Unauthorized pause attempt");
    let result = client.try_emergency_pause_rent(&LEASE_ID, &unauthorized, &reason);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_emergency_pause_rent_already_paused() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Pause rent first time
    let reason1 = String::from_str(&env, "First pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason1);

    // Try to pause again
    let reason2 = String::from_str(&env, "Second pause");
    let result = client.try_emergency_pause_rent(&LEASE_ID, &admin, &reason2);
    assert_eq!(result, Err(Ok(LeaseError::LeaseAlreadyPaused)));
}

#[test]
fn test_emergency_resume_rent() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Fast forward 1 day
    env.ledger().with_mut(|l| l.timestamp = START + 86400);

    // Resume rent
    client.emergency_resume_rent(&LEASE_ID, &admin);

    // Verify lease is no longer paused
    let (paused, _, _, total_paused_duration) = client.get_pause_status(&LEASE_ID).unwrap();
    assert!(!paused);
    assert_eq!(total_paused_duration, 86400); // 1 day

    // Verify lease status changed back to Active
    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.status, LeaseStatus::Active);
}

#[test]
fn test_emergency_resume_rent_not_paused() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Try to resume rent without pausing first
    let result = client.try_emergency_resume_rent(&LEASE_ID, &admin);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotPaused)));
}

#[test]
fn test_rent_calculation_with_pause() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let rent_per_sec = 1i128; // 1 unit per second for easy calculation
    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Fast forward 1000 seconds (should owe 1000 units)
    env.ledger().with_mut(|l| l.timestamp = START + 1000);
    let debt_before_pause = client.check_tenant_default(&LEASE_ID).unwrap();
    assert_eq!(debt_before_pause, 1000);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Fast forward another 1000 seconds while paused
    env.ledger().with_mut(|l| l.timestamp = START + 2000);
    let debt_during_pause = client.check_tenant_default(&LEASE_ID).unwrap();
    // Debt should still be 1000 (no accrual during pause)
    assert_eq!(debt_during_pause, 1000);

    // Resume rent
    client.emergency_resume_rent(&LEASE_ID, &admin);

    // Fast forward another 500 seconds after resume
    env.ledger().with_mut(|l| l.timestamp = START + 2500);
    let debt_after_resume = client.check_tenant_default(&LEASE_ID).unwrap();
    // Debt should be 1000 (before pause) + 500 (after resume) = 1500
    assert_eq!(debt_after_resume, 1500);
}

#[test]
fn test_no_eviction_during_pause() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000, // Eviction threshold is 2000
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: START + 100, // Short grace period
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Pause rent immediately
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Fast forward way past eviction threshold time
    env.ledger().with_mut(|l| l.timestamp = START + 10000);
    
    // Check debt - should not trigger eviction event due to pause
    let events_before = env.events().all().len();
    let _debt = client.check_tenant_default(&LEASE_ID).unwrap();
    let events_after = env.events().all().len();
    
    // No new eviction events should be emitted during pause
    assert_eq!(events_before, events_after);
}

#[test]
fn test_terminate_paused_lease() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    // Setup
    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token,
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: soroban_sdk::Vec::new(&env),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Settle deposit first
    let mut lease = client.get_lease_instance(&LEASE_ID).unwrap();
    // Manually update deposit status for test (in real scenario this would be done through proper flow)
    env.as_contract(&env.register(LeaseContract, ()), || {
        let mut lease = load_lease_instance_by_id(&env, LEASE_ID).unwrap();
        lease.deposit_status = DepositStatus::Settled;
        save_lease_instance(&env, LEASE_ID, &lease);
    });

    // Should be able to terminate paused lease even before end_date
    client.terminate_lease(&LEASE_ID, &landlord);

    // Verify lease was terminated
    let result = client.try_get_lease_instance(&LEASE_ID);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound))); // Archived
}