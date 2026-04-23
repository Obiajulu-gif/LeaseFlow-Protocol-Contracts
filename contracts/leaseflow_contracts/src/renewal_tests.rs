#![cfg(test)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]

use super::*;
use crate::{
    CreateLeaseParams, DataKey, DepositStatus, HistoricalLease, LeaseContract, LeaseContractClient,
    LeaseStatus, LeaseRenewalProposal, RateType,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

const START: u64 = 1711929600;
const END: u64 = 1714521600;
const LEASE_ID: u64 = 1;
const PROPOSAL_DURATION: u64 = 86400; // 24 hours

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
        rent_per_sec: 1_000,
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
    }
}

fn seed_lease(env: &Env, contract_id: &Address, lease_id: u64, lease: &LeaseInstance) {
    env.as_contract(contract_id, || save_lease_instance(env, lease_id, lease));
}

fn read_lease(env: &Env, contract_id: &Address, lease_id: u64) -> Option<LeaseInstance> {
    env.as_contract(contract_id, || load_lease_instance_by_id(env, lease_id))
}

#[test]
fn test_propose_lease_renewal_success() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let new_end_date = END + 30 * 86400; // 30 days extension
    let new_rent_amount = 1_200;
    let new_deposit_amount = 600;
    let new_rent_per_sec = 1_200;
    
    // Propose renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &new_end_date,
        &new_rent_amount,
        &new_deposit_amount,
        &new_rent_per_sec,
        &PROPOSAL_DURATION,
    );
    
    // Check proposal was saved
    let proposal = client.get_renewal_proposal(&LEASE_ID);
    assert_eq!(proposal.lease_id, LEASE_ID);
    assert_eq!(proposal.landlord, landlord);
    assert_eq!(proposal.proposed_end_date, new_end_date);
    assert_eq!(proposal.proposed_rent_amount, new_rent_amount);
    assert_eq!(proposal.proposed_deposit_amount, new_deposit_amount);
    assert_eq!(proposal.proposed_rent_per_sec, new_rent_per_sec);
    assert!(proposal.landlord_signature);
    assert!(!proposal.tenant_signature);
}

#[test]
fn test_propose_lease_renewal_unauthorized() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let result = client.try_propose_lease_renewal(
        &LEASE_ID,
        &unauthorized, // Not the landlord
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    assert_eq!(result, Err(LeaseError::Unauthorised));
}

#[test]
fn test_propose_lease_renewal_invalid_terms() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Test with end date in the past
    let result = client.try_propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &START, // Past end date
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    assert_eq!(result, Err(LeaseError::InvalidRenewalTerms));
    
    // Test with negative amounts
    let result = client.try_propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &-100, // Negative rent
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    assert_eq!(result, Err(LeaseError::InvalidRenewalTerms));
}

#[test]
fn test_accept_renewal_success() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let new_end_date = END + 30 * 86400;
    let new_rent_amount = 1_200;
    let new_deposit_amount = 600;
    let new_rent_per_sec = 1_200;
    
    // Propose renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &new_end_date,
        &new_rent_amount,
        &new_deposit_amount,
        &new_rent_per_sec,
        &PROPOSAL_DURATION,
    );
    
    // Accept renewal
    client.accept_renewal(&LEASE_ID, &tenant);
    
    // Check lease was updated
    let updated_lease = read_lease(&env, &contract_id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.end_date, new_end_date);
    assert_eq!(updated_lease.rent_amount, new_rent_amount);
    assert_eq!(updated_lease.security_deposit, new_deposit_amount);
    assert_eq!(updated_lease.rent_per_sec, new_rent_per_sec);
    
    // Check proposal was cleaned up
    let result = client.try_get_renewal_proposal(&LEASE_ID);
    assert_eq!(result, Err(LeaseError::RenewalNotProposed));
}

#[test]
fn test_accept_renewal_unauthorized() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    
    // Try to accept as unauthorized party
    let result = client.try_accept_renewal(&LEASE_ID, &unauthorized);
    assert_eq!(result, Err(LeaseError::Unauthorised));
}

#[test]
fn test_accept_renewal_no_proposal() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Try to accept without proposal
    let result = client.try_accept_renewal(&LEASE_ID, &tenant);
    assert_eq!(result, Err(LeaseError::RenewalNotProposed));
}

#[test]
fn test_accept_renewal_expired() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose renewal with short duration
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &1, // 1 second duration
    );
    
    // Advance time past expiration
    env.ledger().with_mut(|l| l.timestamp = START + 2);
    
    // Try to accept expired proposal
    let result = client.try_accept_renewal(&LEASE_ID, &tenant);
    assert_eq!(result, Err(LeaseError::RenewalExpired));
}

#[test]
fn test_renewal_deposit_increase() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let new_end_date = END + 30 * 86400;
    let new_rent_amount = 1_200;
    let new_deposit_amount = 800; // Higher deposit
    let new_rent_per_sec = 1_200;
    
    // Propose renewal with higher deposit
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &new_end_date,
        &new_rent_amount,
        &new_deposit_amount,
        &new_rent_per_sec,
        &PROPOSAL_DURATION,
    );
    
    // Accept renewal
    client.accept_renewal(&LEASE_ID, &tenant);
    
    // Check deposit was updated
    let updated_lease = read_lease(&env, &contract_id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.security_deposit, new_deposit_amount);
}

#[test]
fn test_renewal_deposit_decrease() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let new_end_date = END + 30 * 86400;
    let new_rent_amount = 1_200;
    let new_deposit_amount = 300; // Lower deposit
    let new_rent_per_sec = 1_200;
    
    // Propose renewal with lower deposit
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &new_end_date,
        &new_rent_amount,
        &new_deposit_amount,
        &new_rent_per_sec,
        &PROPOSAL_DURATION,
    );
    
    // Accept renewal
    client.accept_renewal(&LEASE_ID, &tenant);
    
    // Check deposit was updated
    let updated_lease = read_lease(&env, &contract_id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.security_deposit, new_deposit_amount);
}

#[test]
fn test_reject_renewal() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    
    // Reject renewal as tenant
    client.reject_renewal(&LEASE_ID, &tenant);
    
    // Check proposal was removed
    let result = client.try_get_renewal_proposal(&LEASE_ID);
    assert_eq!(result, Err(LeaseError::RenewalNotProposed));
}

#[test]
fn test_reject_renewal_unauthorized() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    
    // Try to reject as unauthorized party
    let result = client.try_reject_renewal(&LEASE_ID, &unauthorized);
    assert_eq!(result, Err(LeaseError::Unauthorised));
    
    // Proposal should still exist
    let proposal = client.get_renewal_proposal(&LEASE_ID);
    assert_eq!(proposal.lease_id, LEASE_ID);
}

#[test]
fn test_proposal_replacement() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    let lease = make_lease(&env, &landlord, &tenant);
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose first renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    
    let first_proposal = client.get_renewal_proposal(&LEASE_ID);
    assert_eq!(first_proposal.proposed_rent_amount, 1_200);
    
    // Propose second renewal (should replace first)
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 60 * 86400,
        &1_500,
        &700,
        &1_500,
        &PROPOSAL_DURATION,
    );
    
    let second_proposal = client.get_renewal_proposal(&LEASE_ID);
    assert_eq!(second_proposal.proposed_rent_amount, 1_500);
    assert_eq!(second_proposal.proposed_end_date, END + 60 * 86400);
}

#[test]
fn test_time_based_proration_continuation() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    // Create lease with some rent paid
    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.rent_paid = 500;
    lease.cumulative_payments = 500;
    lease.rent_paid_through = START + 15 * 86400; // 15 days paid
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    let new_end_date = END + 30 * 86400;
    let new_rent_amount = 1_200;
    let new_deposit_amount = 600;
    let new_rent_per_sec = 1_200;
    
    // Propose and accept renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &new_end_date,
        &new_rent_amount,
        &new_deposit_amount,
        &new_rent_per_sec,
        &PROPOSAL_DURATION,
    );
    
    client.accept_renewal(&LEASE_ID, &tenant);
    
    // Check that existing rent payments are preserved
    let updated_lease = read_lease(&env, &contract_id, LEASE_ID).unwrap();
    assert_eq!(updated_lease.rent_paid, 500);
    assert_eq!(updated_lease.cumulative_payments, 500);
    assert_eq!(updated_lease.rent_paid_through, START + 15 * 86400);
    
    // Check that new terms are applied
    assert_eq!(updated_lease.end_date, new_end_date);
    assert_eq!(updated_lease.rent_amount, new_rent_amount);
    assert_eq!(updated_lease.rent_per_sec, new_rent_per_sec);
}

#[test]
fn test_yield_accumulation_preservation() {
    let env = make_env();
    let (contract_id, client) = setup(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    
    // Create lease with yield accumulation
    let mut lease = make_lease(&env, &landlord, &tenant);
    lease.yield_delegation_enabled = true;
    lease.yield_accumulated = 100;
    lease.equity_balance = 50;
    seed_lease(&env, &contract_id, LEASE_ID, &lease);
    
    // Propose and accept renewal
    client.propose_lease_renewal(
        &LEASE_ID,
        &landlord,
        &END + 30 * 86400,
        &1_200,
        &600,
        &1_200,
        &PROPOSAL_DURATION,
    );
    
    client.accept_renewal(&LEASE_ID, &tenant);
    
    // Check that yield-related fields are preserved
    let updated_lease = read_lease(&env, &contract_id, LEASE_ID).unwrap();
    assert!(updated_lease.yield_delegation_enabled);
    assert_eq!(updated_lease.yield_accumulated, 100);
    assert_eq!(updated_lease.equity_balance, 50);
}
