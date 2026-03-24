#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Symbol,
};

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};
    

    #[allow(dead_code)]
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

/// Seconds of lease time granted per unit of funds added (1 day per unit).
pub const SECS_PER_UNIT: u64 = 86_400;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    pub amount: i128,
    pub nft_contract: Option<Address>,  // None if no NFT involved
    pub token_id: Option<u128>,         // None if no NFT involved
    pub active: bool,
    pub expiry_time: u64,
}

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    /// Original function — unchanged behaviour, no NFT required.
    pub fn create_lease(env: Env, landlord: Address, tenant: Address, amount: i128) -> Symbol {
    /// Initializes a lease between a landlord and a tenant.
    /// `lease_id` uniquely identifies the lease in storage.
    /// `duration` sets the initial lease duration in seconds.
    pub fn create_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        amount: i128,
        duration: u64,
    ) -> Symbol {
        let expiry_time = env.ledger().timestamp().saturating_add(duration);
        let lease = Lease {
            landlord,
            tenant,
            amount,
            nft_contract: None,
            token_id: None,
            active: true,
        };
        env.storage()
            .instance()
            .set(&symbol_short!("lease"), &lease);
        symbol_short!("created")
    }

    /// New function — same as above but also transfers an NFT from landlord to tenant.
    pub fn create_lease_with_nft(
        env: Env,
        landlord: Address,
        tenant: Address,
        amount: i128,
        nft_contract: Address,
        token_id: u128,
    ) -> Symbol {
        landlord.require_auth();

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract);
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &tenant,
            &token_id,
        );

        let lease = Lease {
            landlord,
            tenant,
            amount,
            nft_contract: Some(nft_contract),
            token_id: Some(token_id),
            active: true,
            expiry_time,
        };
        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
    }

    pub fn get_lease(env: Env) -> Lease {
    /// Returns the lease details for the given `lease_id`.
    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found")
    }

    /// Adds funds to an existing lease, extending `expiry_time` proportionally.
    /// Each unit of `amount` extends the lease by `SECS_PER_UNIT` seconds.
    /// Requires authorization from the tenant.
    pub fn add_funds(env: Env, lease_id: Symbol, amount: i128) -> Symbol {
        assert!(amount > 0, "amount must be positive");

        let mut lease: Lease = env
            .storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found");

        lease.tenant.require_auth();

        let extra_secs = (amount as u64).saturating_mul(SECS_PER_UNIT);
        lease.amount = lease.amount.saturating_add(amount);
        lease.expiry_time = lease.expiry_time.saturating_add(extra_secs);

        env.storage().instance().set(&lease_id, &lease);

        symbol_short!("extended")
    }
}

mod test;