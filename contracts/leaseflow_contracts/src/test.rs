#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, Symbol, symbol_short, BytesN,
};
use crate::{LeaseContract, LeaseContractClient, LeaseStatus, MaintenanceStatus, DepositStatus, CreateLeaseParams, RateType, HistoricalLease, DataKey, 
    MaintenanceIssueReported, RepairProofSubmitted, MaintenanceVerified, LeaseStarted, LeaseTerminated, DepositReleasePartial};

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
    client.set_inspector(&LEASE_ID, &landlord, &inspector);

    // 1. Tenant reports issue
    client.report_maintenance_issue(&LEASE_ID, &tenant);
    
    // 2. Tenant pays rent - it should be withheld
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
    // 3. Landlord submits repair proof
    let proof_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.submit_repair_proof(&LEASE_ID, &landlord, &proof_hash);
    
    // 4. Inspector verifies repair
    client.verify_repair(&LEASE_ID, &inspector);
    
    let lease = client.get_lease_instance(&LEASE_ID);
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
    client.set_lease_instance_buyout_price(&LEASE_ID, &landlord, &1000);
    
    client.pay_lease_instance_rent(&LEASE_ID, &1000);
    
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
    let refund = client.conclude_lease(&LEASE_ID, &landlord, &500);
    assert_eq!(refund, 1500); // 2000 - 500
}

#[test]
fn test_dispute_resolution_flow() {
    let env = make_env();
    let (id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let admin = Address::generate(&env);

    let params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000,
        deposit_amount: 5000, // Large deposit for splitting
        security_deposit: 500,
        start_date: START,
        end_date: END,
        property_uri: String::from_str(&env, "ipfs://test"),
    };

    client.create_lease_instance(&LEASE_ID, &landlord, &params);
    client.set_admin(&admin);

    // 1. Tenant disputes deposit
    client.dispute_deposit(&LEASE_ID, &tenant);
    let lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(lease.deposit_status, DepositStatus::Disputed);

    // 2. Admin resolves dispute - 30% landlord (3000 bps), 70% tenant
    let resolution = client.resolve_dispute(&LEASE_ID, &3000);
    
    // Verifying split math: 5000 * 3000 / 10000 = 1500
    assert_eq!(resolution.landlord_amount, 1500);
    assert_eq!(resolution.tenant_amount, 3500);
    
    // Total MUST equal 5000
    assert_eq!(resolution.landlord_amount + resolution.tenant_amount, 5000);

    // 3. Mark lease as terminated
    let final_lease = client.get_lease_instance(&LEASE_ID);
    assert_eq!(final_lease.status, LeaseStatus::Terminated);
    assert_eq!(final_lease.deposit_status, DepositStatus::Settled);
}
