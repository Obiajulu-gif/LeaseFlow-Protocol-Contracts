#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, Vec,
};

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

fn create_test_lease(env: &Env, client: &LeaseContractClient, admin: &Address, landlord: &Address, tenant: &Address, token: &Address) {
    client.set_admin(admin);
    client.add_allowed_asset(admin, token);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(env, "ipfs://test"),
        payment_token: token.clone(),
        rent_per_sec: 1,
        grace_period_end: END,
        late_fee_flat: 100,
        late_fee_per_sec: 2,
        arbitrators: Vec::new(env),
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 0,
        swap_path: soroban_sdk::Vec::new(env),
        early_termination_fee_bps: None,
        fixed_penalty: None,
    };

    client.create_lease_instance(&LEASE_ID, landlord, &params);
}

#[test]
fn test_emergency_pause_rent_by_admin() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

    // Test admin can pause rent
    let reason = String::from_str(&env, "Flood damage - property uninhabitable");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Verify lease is paused
    let (paused, pause_reason, paused_at, _) = client.get_pause_status(&LEASE_ID);
    assert!(paused);
    assert_eq!(pause_reason.unwrap(), reason);
    assert_eq!(paused_at.unwrap(), START);

    // Verify lease status changed to Paused
    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.status, LeaseStatus::Paused);
}

#[test]
fn test_emergency_pause_rent_by_landlord() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

    // Test landlord can pause rent
    let reason = String::from_str(&env, "Earthquake - building condemned");
    client.emergency_pause_rent(&LEASE_ID, &landlord, &reason);

    // Verify lease is paused
    let (paused, pause_reason, _, _) = client.get_pause_status(&LEASE_ID);
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

    client.set_admin(&admin);
    client.add_allowed_asset(&admin, &token);

    let mut arbitrators = Vec::new(&env);
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
        fixed_penalty: None,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Test arbitrator can pause rent
    let reason = String::from_str(&env, "Hurricane - mandatory evacuation");
    client.emergency_pause_rent(&LEASE_ID, &arbitrator, &reason);

    // Verify lease is paused
    let (paused, _, _, _) = client.get_pause_status(&LEASE_ID);
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

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

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

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

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

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Fast forward 1 day
    env.ledger().with_mut(|l| l.timestamp = START + 86400);

    // Resume rent
    client.emergency_resume_rent(&LEASE_ID, &admin);

    // Verify lease is no longer paused
    let (paused, _, _, total_paused_duration) = client.get_pause_status(&LEASE_ID);
    assert!(!paused);
    assert_eq!(total_paused_duration, 86400); // 1 day

    // Verify lease status changed back to Active
    let lease = client.get_lease_instance(&LEASE_ID);
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

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

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
        arbitrators: Vec::new(&env),
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        early_termination_fee_bps: None,
        fixed_penalty: None,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    // Fast forward 1000 seconds (should owe 1000 units)
    env.ledger().with_mut(|l| l.timestamp = START + 1000);
    let debt_before_pause = client.check_tenant_default(&LEASE_ID);
    assert_eq!(debt_before_pause, 1000);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Fast forward another 1000 seconds while paused
    env.ledger().with_mut(|l| l.timestamp = START + 2000);
    let debt_during_pause = client.check_tenant_default(&LEASE_ID);
    // Debt should still be 1000 (no accrual during pause)
    assert_eq!(debt_during_pause, 1000);

    // Resume rent
    client.emergency_resume_rent(&LEASE_ID, &admin);

    // Fast forward another 500 seconds after resume
    env.ledger().with_mut(|l| l.timestamp = START + 2500);
    let debt_after_resume = client.check_tenant_default(&LEASE_ID);
    // Debt should be 1000 (before pause) + 500 (after resume) = 1500
    assert_eq!(debt_after_resume, 1500);
}

#[test]
fn test_terminate_paused_lease() {
    let env = make_env();
    let (id, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

    // Pause rent
    let reason = String::from_str(&env, "Emergency pause");
    client.emergency_pause_rent(&LEASE_ID, &admin, &reason);

    // Settle deposit first (simulate proper deposit settlement)
    env.as_contract(&id, || {
        let mut lease = load_lease_instance_by_id(&env, LEASE_ID).unwrap();
        lease.deposit_status = DepositStatus::Settled;
        save_lease_instance(&env, LEASE_ID, &lease);
    });

    // Should be able to terminate paused lease even before end_date
    client.terminate_lease(&LEASE_ID, &landlord);

    // Verify lease was terminated (archived)
    let result = client.try_get_lease_instance(&LEASE_ID);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound))); // Archived
}

#[test]
fn test_autopay_authorization() {
    let env = make_env();
    let (_, client) = setup(&env);
    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    create_test_lease(&env, &client, &admin, &landlord, &tenant, &token);

    // Test rent pull authorization
    let authorized_amount = 1000_0000000;
    client.authorize_rent_pull(&LEASE_ID, &tenant, &authorized_amount, &None);

    // Verify authorization status
    let (auth_amount, last_pull, cycle_duration, _next_available) = 
        client.get_rent_pull_status(&LEASE_ID);

    assert_eq!(auth_amount, Some(authorized_amount));
    assert_eq!(last_pull, None);
    assert_eq!(cycle_duration, 2_592_000u64); // Default 30 days
}