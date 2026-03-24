#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_lease() {
    let env = Env::default();
    let contract_id = env.register(LeaseContract, ());
    let client = LeaseContractClient::new(&env, &contract_id);

    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let amount = 1000i128;

    client.create_lease(&landlord, &tenant, &amount);
    let lease = client.get_lease();

    assert_eq!(lease.landlord, landlord);
    assert_eq!(lease.tenant, tenant);
    assert_eq!(lease.amount, amount);
    assert!(lease.active);
}
