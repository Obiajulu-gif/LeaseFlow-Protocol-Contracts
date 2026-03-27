#![cfg(test)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use super::*;
use crate::{
    CreateLeaseParams, DataKey, DepositStatus, HistoricalLease, LeaseContract, LeaseContractClient,
    LeaseStatus, MaintenanceStatus, RateType,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

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
        expiry_time: END,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: START,
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
        maintenance_status: MaintenanceStatus::None,
        withheld_rent: 0,
        repair_proof_hash: None,
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

    let res = client.try_initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &5000,
        &10000,
        &31536000,
        &uri,
        &volatile_token,
    );
    assert!(res.is_err());

    client.initialize_lease(
        &lease_id, &landlord, &tenant, &5000, &10000, &31536000, &uri, &usdc,
    );
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
    client.initialize_lease(
        &lease_id,
        &landlord,
        &tenant,
        &5000,
        &10000,
        &31536000,
        &String::from_str(&env, "ipfs://test"),
        &token,
    );

    client.activate_lease(&lease_id, &tenant);
    client.pay_rent(&lease_id, &5000);

    let month = 1u32;
    let amount_paid = 5000i128;
    client.pay_rent_receipt(&lease_id, &month, &amount_paid);

    let receipt = client.get_receipt(&lease_id, &month);
    assert_eq!(receipt.lease_id, lease_id);
    assert_eq!(receipt.month, month);
    assert_eq!(receipt.amount, amount_paid);

    client.extend_ttl(&lease_id);
}

#[test]
fn test_terminate_lease_before_end_date_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END - 1);

    let result = client.try_terminate_lease(&LEASE_ID, &landlord);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotExpired)));
}

#[test]
fn test_terminate_lease_with_outstanding_rent_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.rent_paid_through = END - 1;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&LEASE_ID, &landlord);
    assert!(result.is_err());
}

#[test]
fn test_terminate_lease_with_unsettled_deposit_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&LEASE_ID, &landlord);
    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
}

#[test]
fn test_terminate_lease_with_disputed_deposit_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Disputed;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&LEASE_ID, &landlord);
    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
}

#[test]
fn test_terminate_lease_unauthorised_caller_fails() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let stranger = Address::generate(&env);

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&LEASE_ID, &stranger);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_terminate_lease_not_found_fails() {
    let env = make_env();
    let (_, client) = setup(&env);
    let caller = Address::generate(&env);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_terminate_lease(&99u64, &caller);
    assert_eq!(result, Err(Ok(LeaseError::LeaseNotFound)));
}

#[test]
fn test_terminate_lease_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Settled;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    client.terminate_lease(&LEASE_ID, &landlord);
    assert!(read_lease(&env, &id, LEASE_ID).is_none());
}

#[test]
fn test_activate_lease_success() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    client.create_lease(&landlord, &tenant, &1000i128, &token);
    let result = client.activate_lease(&symbol_short!("lease"), &tenant);

    assert_eq!(result, symbol_short!("active"));
}

#[test]
fn test_reclaim_asset_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let reason = String::from_str(&env, "Lease expired - asset returned");

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));
    client.reclaim_asset(&LEASE_ID, &landlord, &reason);
}

#[test]
fn test_reclaim_asset_unauthorized() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let reason = String::from_str(&env, "Unauthorized attempt");

    seed_lease(&env, &id, LEASE_ID, &make_lease(&env, &landlord, &tenant));

    let result = client.try_reclaim_asset(&LEASE_ID, &unauthorized, &reason);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_reclaim_success() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_amount = 0;
    seed_lease(&env, &id, LEASE_ID, &lease);

    client.reclaim(&LEASE_ID, &landlord);

    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert!(!updated_lease.active);
}

#[test]
fn test_reclaim_fails_when_balance_not_zero() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_amount = 100;
    seed_lease(&env, &id, LEASE_ID, &lease);

    let result = client.try_reclaim(&LEASE_ID, &landlord);
    assert_eq!(result, Err(Ok(LeaseError::DepositNotSettled)));
}

// ---------------------------------------------------------------------------
// NFT Escrow Tests
// ---------------------------------------------------------------------------

#[contractclient(name = "MockNftClient")]
pub trait MockNftInterface {
    fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    fn owner_of(env: Env, token_id: u128) -> Address;
}

#[test]
fn test_create_lease_with_nft_escrows_to_contract() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token = Address::generate(&env);
    let token_id: u128 = 123;

    let lease_id = symbol_short!("lease_01");
    let result = client.create_lease_with_nft(
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

    assert_eq!(result, symbol_short!("created"));

    let usage_rights = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights.is_some());

    let rights = usage_rights.unwrap();
    assert_eq!(rights.renter, tenant);
    assert_eq!(rights.nft_contract, nft_contract);
    assert_eq!(rights.token_id, token_id);
    assert_eq!(rights.lease_id, lease_id);
}

#[test]
fn test_end_lease_returns_nft_to_landlord() {
    let env = make_env();
    let (_, client) = setup(&env);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    let token = Address::generate(&env);
    let token_id: u128 = 456;

    let lease_id = symbol_short!("lease_01");
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

    let usage_rights_before = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_before.is_some());

    let result = client.end_lease(&lease_id, &landlord);
    assert_eq!(result, symbol_short!("ended"));

    let usage_rights_after = client.check_usage_rights(&nft_contract, &token_id, &tenant);
    assert!(usage_rights_after.is_none());
}

#[test]
fn test_maintenance_flow_with_events() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let inspector = Address::generate(&env);
    let token = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_inspector(&LEASE_ID, &landlord, &inspector);
    client.report_maintenance_issue(&LEASE_ID, &tenant);

    client.pay_lease_instance_rent(&LEASE_ID, &tenant, &1000);

    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.rent_paid, 1000);
}

#[test]
fn test_lease_instance_buyout() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &3000i128);

    client.pay_lease_instance_rent(&LEASE_ID, &tenant, &1000i128);
    client.pay_lease_instance_rent(&LEASE_ID, &tenant, &1000i128);
    client.pay_lease_instance_rent(&LEASE_ID, &tenant, &1000i128);

    assert!(read_lease(&env, &id, LEASE_ID).is_none());

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

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &3000i128);

    client.pay_lease_instance_rent(&LEASE_ID, &tenant, &1000i128);

    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.cumulative_payments, 1000i128);
    assert!(lease.active);
}

// ---------------------------------------------------------------------------
// conclude_lease tests
// ---------------------------------------------------------------------------

#[test]
fn test_conclude_lease_no_damages_full_refund() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.conclude_lease(&LEASE_ID, &landlord, &0i128);
    assert_eq!(result, 500);

    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert_eq!(updated_lease.deposit_status, DepositStatus::Settled);
}

#[test]
fn test_conclude_lease_with_damages_partial_refund() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.conclude_lease(&LEASE_ID, &landlord, &200i128);
    assert_eq!(result, 300);

    let updated_lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.status, LeaseStatus::Terminated);
    assert_eq!(updated_lease.deposit_status, DepositStatus::Settled);
}

#[test]
fn test_conclude_lease_tenant_unauthorised() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_conclude_lease(&LEASE_ID, &tenant, &100i128);
    assert_eq!(result, Err(Ok(LeaseError::Unauthorised)));
}

#[test]
fn test_conclude_lease_negative_deduction() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &-100i128);
    assert_eq!(result, Err(Ok(LeaseError::InvalidDeduction)));
}

#[test]
fn test_conclude_lease_excessive_deduction() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.deposit_status = DepositStatus::Held;
    lease.status = LeaseStatus::Active;
    seed_lease(&env, &id, LEASE_ID, &lease);
    env.ledger().with_mut(|l| l.timestamp = END + 1);

    let result = client.try_conclude_lease(&LEASE_ID, &landlord, &600i128);
    assert_eq!(result, Err(Ok(LeaseError::InvalidDeduction)));
}

#[test]
fn test_create_lease_instance_with_security_deposit() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 2000,
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: token.clone(),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    let lease = read_lease(&env, &id, LEASE_ID).unwrap();
    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.security_deposit, 500);
    assert_eq!(lease.status, LeaseStatus::Pending);
}

#[test]
fn test_tenant_default_scenario_3_months_non_payment() {
    let env = make_env();
    let (_, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let token = Address::generate(&env);

    let month_in_secs: u64 = 2_592_000;
    let rent_amount = 1000i128;
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
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);

    env.ledger()
        .with_mut(|l| l.timestamp = start_date + month_in_secs + 1);
    let debt_1 = client.check_tenant_default(&LEASE_ID);
    assert!(debt_1 > 0);

    let three_months = start_date + month_in_secs * 3;
    env.ledger().with_mut(|l| l.timestamp = three_months);
    let debt_3 = client.check_tenant_default(&LEASE_ID);
    assert!(debt_3 > rent_amount * 2);
}

#[test]
fn test_double_sign_prevention() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let payment_token = Address::generate(&env);

    let lease_id = 1u64;

    let mut params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1500,
        deposit_amount: 1500,
        security_deposit: 1500,
        start_date: env.ledger().timestamp(),
        end_date: env.ledger().timestamp() + (30 * 86400),
        property_uri: String::from_str(&env, "ipfs://QmLeaseDoc"),
        payment_token: payment_token.clone(),
    };

    let result = client.try_create_lease_instance(&lease_id, &landlord, &params);
    assert!(result.is_ok(), "Initial lease creation should succeed");

    params.rent_amount = 500;

    let malicious_result = client.try_create_lease_instance(&lease_id, &landlord, &params);

    assert!(
        malicious_result.is_err(),
        "Contract must reject attempts to overwrite an existing lease"
    );

    let active_lease = client.get_lease_instance(&lease_id);
    assert_eq!(
        active_lease.rent_amount, 1500,
        "Rent amount should remain untouched at 1500"
    );
}
