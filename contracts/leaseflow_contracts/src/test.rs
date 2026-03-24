#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    testutils::Address as _,
    Address, Env,
};

// ── Mock NFT Contract ─────────────────────────────────────────────────────────
//
// This pretends to be a real NFT contract for testing purposes.
// It records the last transfer so we can assert it happened correctly.

#[contract]
pub struct MockNftContract;

// We store the last transfer details in contract storage so the test can read them.
#[contracttype]
pub struct TransferRecord {
    pub from: Address,
    pub to: Address,
    pub token_id: u128,
}

#[contractimpl]
impl MockNftContract {
    pub fn transfer_from(
        env: Env,
        _spender: Address, // we ignore spender in the mock, a real contract would check it
        from: Address,
        to: Address,
        token_id: u128,
    ) {
        // Record the transfer so we can assert on it in the test
        env.storage().instance().set(
            &symbol_short!("last_xfr"),
            &TransferRecord { from, to, token_id },
        );
    }

    // Helper to read back the last recorded transfer
    pub fn get_last_transfer(env: Env) -> TransferRecord {
        env.storage()
            .instance()
            .get(&symbol_short!("last_xfr"))
            .expect("No transfer recorded")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

#[test]
fn test_lease_with_nft() {
    let env = Env::default();
    // In tests we need to disable auth checks so require_auth() doesn't fail
    env.mock_all_auths();

    // Deploy the mock NFT contract
    let nft_id = env.register(MockNftContract, ());
    let nft_client = MockNftContractClient::new(&env, &nft_id);

    // Deploy the lease contract
    let lease_id = env.register(LeaseContract, ());
    let lease_client = LeaseContractClient::new(&env, &lease_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token_id: u128 = 42;
    let rent_amount: i128 = 1000;

    // Create the lease — this should trigger transfer_from on the mock NFT
    lease_client.create_lease_with_nft(
    &landlord,
    &tenant,
    &rent_amount,
    &nft_id,
    &token_id,
);
    let lease_id = symbol_short!("lease");
    let amount = 1000i128;
    let duration = 86_400u64; // 1 day

    client.create_lease(&lease_id, &landlord, &tenant, &amount, &duration);
    let lease = client.get_lease(&lease_id);

    // ── Assert: lease was stored correctly ────────────────────────────────────
    let lease = lease_client.get_lease();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, rent_amount);
    assert_eq!(lease.nft_contract, Some(nft_id.clone()));
    assert_eq!(lease.token_id, Some(token_id));
    assert!(lease.active);

    // ── Assert: transfer_from was called with the right arguments ─────────────
    // This is the key acceptance criterion: verify transfer_from works correctly
    let transfer = nft_client.get_last_transfer();
    assert_eq!(transfer.from, landlord,   "NFT should move FROM the landlord");
    assert_eq!(transfer.to, tenant,       "NFT should move TO the tenant");
    assert_eq!(transfer.token_id, token_id, "Token ID should match");
    assert_eq!(lease.expiry_time, duration); // ledger timestamp starts at 0 in tests
}

#[test]
fn test_add_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let lease_id = symbol_short!("lease1");
    let initial_amount = 1000i128;
    let duration = 86_400u64; // 1 day

    client.create_lease(&lease_id, &landlord, &tenant, &initial_amount, &duration);

    let before = client.get_lease(&lease_id);
    let added_amount = 500i128;

    client.add_funds(&lease_id, &added_amount);

    let after = client.get_lease(&lease_id);

    assert_eq!(after.amount, initial_amount + added_amount);
    assert_eq!(
        after.expiry_time,
        before.expiry_time + (added_amount as u64 * SECS_PER_UNIT)
    );
    assert_eq!(after.landlord, landlord);
    assert_eq!(after.tenant, tenant);
    assert!(after.active);
}

#[test]
fn test_original_lease_fields_unchanged() {
    let env = Env::default();

    let lease_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &lease_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &500i128);

    let lease = client.get_lease();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, 500);
    assert!(lease.active);
    assert_eq!(lease.nft_contract, None);
    assert_eq!(lease.token_id, None);
}