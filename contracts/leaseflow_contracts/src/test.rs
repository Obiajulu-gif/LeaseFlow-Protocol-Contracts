#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, Symbol, symbol_short, BytesN, contract, contractimpl,
};
use crate::{LeaseContract, LeaseContractClient, LeaseStatus, MaintenanceStatus, DepositStatus, CreateLeaseParams, RateType, HistoricalLease, DataKey, 
    MaintenanceIssueReported, RepairProofSubmitted, MaintenanceVerified, LeaseStarted, LeaseTerminated, DepositReleasePartial};

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
        rent_paid_through: START,
        deposit_status: DepositStatus::Held,
        buyout_price: None,
        cumulative_payments: 0,
        maintenance_status: MaintenanceStatus::None,
        repair_proof_hash: None,
        withheld_rent: 0,
        inspector: None,
        payment_token: Address::generate(env),
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
        payment_token: token,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_inspector(&LEASE_ID, &landlord, &inspector);
    client.report_maintenance_issue(&LEASE_ID, &tenant);
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.withheld_rent, 1000);
}

#[test]
fn test_lease_instance_buyout() {
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
        payment_token: token,
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &1000);
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
    // Result should be terminated (archived means not found in instance storage)
    let res = client.try_get_lease_instance(&LEASE_ID);
    assert!(res.is_err());
}
