#![cfg(test)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use super::*;
use crate::{
    CreateLeaseParams, DataKey, DepositStatus, HistoricalLease, LeaseContract, LeaseContractClient,
    LeaseStatus, MaintenanceStatus, RateType, SubletStatus, UtilityBillStatus, LeaseInstance,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

const START: u64 = 1711929600;
const END: u64 = 1714521600; // 30 days after START
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

fn make_lease_with_early_termination_fees(
    env: &Env,
    landlord: &Address,
    tenant: &Address,
    early_termination_fee_bps: Option<u32>,
    fixed_penalty: Option<i128>,
) -> LeaseInstance {
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
        expiry_time: END,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: START,
        deposit_status: DepositStatus::Held,
        rent_per_sec: 1, // 1 unit per second for easy calculation
        grace_period_end: END,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        flat_fee_applied: false,
        seconds_late_charged: 0,
        withdrawal_address: None,
        rent_withdrawn: 0,
        arbitrators: soroban_sdk::Vec::new(env),
        paused: false,
        pause_reason: None,
        paused_at: None,
        pause_initiator: None,
        total_paused_duration: 0,
        rent_pull_authorized_amount: None,
        last_rent_pull_timestamp: None,
        billing_cycle_duration: 2_592_000,
        yield_delegation_enabled: false,
        yield_accumulated: 0,
        equity_balance: 0,
        equity_percentage_bps: 0,
        had_late_payment: false,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        early_termination_fee_bps,
        fixed_penalty,
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_early_termination_10_percent_completion_percentage_fee() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);
    let admin = Address::generate(&env);

    // Setup lease with 10% early termination fee
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000), // 10% fee in basis points
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 10% completion (3 days into 30-day lease)
    let ten_percent_time = START + (END - START) / 10;
    env.ledger().with_mut(|l| l.timestamp = ten_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());

    // Verify historical lease exists
    let historical: HistoricalLease = env.as_contract(&id, || {
        env.storage()
            .persistent()
            .get(&DataKey::HistoricalLease(LEASE_ID))
            .expect("HistoricalLease not found")
    });
    assert_eq!(historical.lease.status, LeaseStatus::Terminated);
    assert_eq!(historical.terminated_by, tenant);
}

#[test]
fn test_early_termination_50_percent_completion_percentage_fee() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease with 20% early termination fee
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(2000), // 20% fee in basis points
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 50% completion (15 days into 30-day lease)
    let fifty_percent_time = START + (END - START) / 2;
    env.ledger().with_mut(|l| l.timestamp = fifty_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_early_termination_90_percent_completion_percentage_fee() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease with 15% early termination fee
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1500), // 15% fee in basis points
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 90% completion (27 days into 30-day lease)
    let ninety_percent_time = START + (END - START) * 9 / 10;
    env.ledger().with_mut(|l| l.timestamp = ninety_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_early_termination_fixed_penalty() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease with fixed penalty
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        None,
        Some(200), // Fixed penalty of 200 units
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 30% completion
    let thirty_percent_time = START + (END - START) * 3 / 10;
    env.ledger().with_mut(|l| l.timestamp = thirty_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_early_termination_no_penalty() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease with no penalty
    let lease = make_lease_with_early_termination_fees(&env, &landlord, &tenant, None, None);
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 25% completion
    let twenty_five_percent_time = START + (END - START) / 4;
    env.ledger().with_mut(|l| l.timestamp = twenty_five_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_early_termination_penalty_exceeds_deposit() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease with high percentage fee that will exceed deposit
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(5000), // 50% fee
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to early in lease (maximum remaining value)
    let early_time = START + 86400; // 1 day into lease
    env.ledger().with_mut(|l| l.timestamp = early_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());

    // Verify tenant is flagged as defaulted
    let defaulted_key = DataKey::DefaultedBalance(tenant.clone());
    let is_defaulted: bool = env.as_contract(&id, || {
        env.storage()
            .persistent()
            .get(&defaulted_key)
            .unwrap_or(false)
    });
    assert!(is_defaulted);
}

#[test]
fn test_early_termination_unauthorized_caller_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    // Setup lease
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000),
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Try early termination with unauthorized caller
    let result = client.try_execute_early_termination(&LEASE_ID, &unauthorized);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_early_termination_inactive_lease_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup inactive lease
    let mut lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000),
        None,
    );
    lease.status = LeaseStatus::Expired;
    lease.active = false;
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Try early termination on inactive lease
    let result = client.try_execute_early_termination(&LEASE_ID, &tenant);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

#[test]
fn test_early_termination_after_end_date_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    // Setup lease
    let lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000),
        None,
    );
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time past end date
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    // Try early termination after end date
    let result = client.try_execute_early_termination(&LEASE_ID, &tenant);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotExpired)));
}

#[test]
fn test_early_termination_with_nft() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token_id: u128 = 123;

    // Setup lease with NFT
    let mut lease = make_lease_with_early_termination_fees(
        &env,
        &landlord,
        &tenant,
        Some(1000),
        None,
    );
    lease.nft_contract = Some(nft_contract.clone());
    lease.token_id = Some(token_id);
    seed_lease(&env, &id, LEASE_ID, &lease);

    // Advance time to 40% completion
    let forty_percent_time = START + (END - START) * 4 / 10;
    env.ledger().with_mut(|l| l.timestamp = forty_percent_time);

    // Execute early termination
    client.execute_early_termination(&LEASE_ID, &tenant);

    // Verify lease is terminated
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_create_lease_instance_with_early_termination_fees() {
    let env = make_env();
    let (_, client) = setup(&env);
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
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
        early_termination_fee_bps: Some(1500), // 15% fee
        fixed_penalty: None,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.early_termination_fee_bps, Some(1500));
    assert_eq!(lease.fixed_penalty, None);
}

#[test]
fn test_create_lease_instance_with_fixed_penalty() {
    let env = make_env();
    let (_, client) = setup(&env);
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
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(&env),
        early_termination_fee_bps: None,
        fixed_penalty: Some(300), // Fixed penalty of 300
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    let lease = client.get_lease_instance(&LEASE_ID).unwrap();
    assert_eq!(lease.early_termination_fee_bps, None);
    assert_eq!(lease.fixed_penalty, Some(300));
}
