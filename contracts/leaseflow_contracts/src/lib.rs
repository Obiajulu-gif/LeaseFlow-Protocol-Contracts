#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub amount: i128,
    pub active: bool,
}

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Initializes a simple lease between a landlord and a tenant.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
        let lease = Lease {
            landlord,
            tenant,
            amount,
            active: true,
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
    }

    /// Returns the current lease details stored in the contract.
    pub fn get_lease(env: Env) -> Lease {
        env.storage()
            .instance()
            .get(&symbol_short!("lease"))
            .expect("Lease not found")
    }
}

mod test;
