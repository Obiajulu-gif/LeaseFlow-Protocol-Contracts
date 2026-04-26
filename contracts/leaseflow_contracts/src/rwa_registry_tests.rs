use soroban_sdk::{contractclient, Address, Env, String, BytesN, Vec};
use crate::{
    LeaseContract, LeaseError, CreateLeaseParams, LeaseStatus, 
    AssetOwnershipVerified
};

#[contractclient(name = "MockRWARegistryClient")]
pub trait MockRWARegistryInterface {
    fn new(env: Env, admin: Address);
    fn get_registry_info(env: Env) -> Result<crate::mock_rwa_registry::RegistryInfo, u32>;
    fn verify_ownership(env: Env, asset_id: u128, claimed_owner: Address) -> bool;
    fn freeze_asset(env: Env, asset_id: u128, freezer: Address) -> Result<BytesN<32>, u32>;
    fn thaw_asset(env: Env, asset_id: u128, freezer: Address, freeze_proof: BytesN<32>) -> Result<(), u32>;
    fn is_asset_frozen(env: Env, asset_id: u128) -> bool;
    fn mint_asset(env: Env, asset_id: u128, owner: Address, metadata_uri: String) -> Result<(), u32>;
    fn transfer_asset(env: Env, asset_id: u128, from: Address, to: Address) -> Result<(), u32>;
    fn set_whitelisted(env: Env, whitelisted: bool);
    fn spoof_ownership(env: Env, asset_id: u128, claimed_owner: Address) -> bool;
}

#[contractclient(name = "LeaseContractClient")]
pub trait LeaseContractInterface {
    fn initialize(env: Env, admin: Address);
    fn add_whitelisted_registry(env: Env, admin: Address, registry_address: Address);
    fn create_lease_instance(env: Env, lease_id: u64, landlord: Address, params: CreateLeaseParams);
    fn get_lease_instance(env: Env, lease_id: u64) -> crate::LeaseInstance;
    fn terminate_lease(env: Env, lease_id: u64, caller: Address);
    fn settle_deposit(env: Env, lease_id: u64, caller: Address, tenant_refund: i128, landlord_payout: i128, dao_payout: i128);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::symbol_short;

    #[test]
    fn test_successful_rwa_verification_and_freeze() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy mock RWA registry
        let registry_address = env.register_contract(None, MockRWARegistry);
        let registry_client = MockRWARegistryClient::new(&env, &registry_address);
        
        let admin = Address::generate(&env);
        registry_client.new(&admin);

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        // Set admin for lease contract
        lease_client.initialize(&admin);

        // Whitelist the registry
        lease_client.add_whitelisted_registry(&admin, &registry_address);

        // Mint an asset for the landlord
        let landlord = Address::generate(&env);
        let asset_id = 12345u128;
        let metadata_uri = String::from_str(&env, "ipfs://QmTest");
        registry_client.mint_asset(&asset_id, &landlord, &metadata_uri);

        // Create lease with RWA verification
        let tenant = Address::generate(&env);
        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30, // 30 days
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30), // Daily rate
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: Some(registry_address.clone()),
            asset_id: Some(asset_id),
        };

        // Create lease - should succeed with RWA verification
        lease_client.create_lease_instance(&lease_id, &landlord, &params);

        // Verify lease was created with RWA fields
        let lease = lease_client.get_lease_instance(&lease_id);
        assert_eq!(lease.asset_registry_address, Some(registry_address));
        assert_eq!(lease.asset_id, Some(asset_id));
        assert!(lease.asset_ownership_verified);
        assert!(lease.asset_freeze_proof.is_some());

        // Verify asset is frozen
        assert!(registry_client.is_asset_frozen(&asset_id));

        // Verify AssetOwnershipVerified event was emitted
        let events = env.events().all();
        let verification_found = events.iter().any(|event| {
            if let Some(verification) = event.as_contract_event::<AssetOwnershipVerified>() {
                verification.lease_id == lease_id 
                    && verification.registry_address == registry_address
                    && verification.asset_id == asset_id
                    && verification.owner == landlord
            } else {
                false
            }
        });
        assert!(verification_found);
    }

    #[test]
    fn test_lease_creation_fails_with_unowned_asset() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy mock RWA registry
        let registry_address = env.register_contract(None, MockRWARegistry);
        let registry_client = MockRWARegistryClient::new(&env, &registry_address);
        
        let admin = Address::generate(&env);
        registry_client.new(&admin);

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        // Set admin and whitelist registry
        lease_client.initialize(&admin);
        lease_client.add_whitelisted_registry(&admin, &registry_address);

        // Mint an asset for someone else (not the landlord)
        let asset_owner = Address::generate(&env);
        let landlord = Address::generate(&env);
        let asset_id = 12345u128;
        let metadata_uri = String::from_str(&env, "ipfs://QmTest");
        registry_client.mint_asset(&asset_id, &asset_owner, &metadata_uri);

        // Try to create lease with unowned asset
        let tenant = Address::generate(&env);
        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30,
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30),
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: Some(registry_address.clone()),
            asset_id: Some(asset_id),
        };

        // Should fail with AssetNotOwned error
        let result = lease_client.try_create_lease_instance(&lease_id, &landlord, &params);
        assert_eq!(result.error(), Some(Ok(LeaseError::AssetNotOwned)));
    }

    #[test]
    fn test_lease_creation_fails_with_non_whitelisted_registry() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy mock RWA registry but don't whitelist it
        let registry_address = env.register_contract(None, MockRWARegistry);
        let registry_client = MockRWARegistryClient::new(&env, &registry_address);
        
        let admin = Address::generate(&env);
        registry_client.new(&admin);

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        lease_client.initialize(&admin);

        // Mint an asset
        let landlord = Address::generate(&env);
        let asset_id = 12345u128;
        let metadata_uri = String::from_str(&env, "ipfs://QmTest");
        registry_client.mint_asset(&asset_id, &landlord, &metadata_uri);

        // Try to create lease with non-whitelisted registry
        let tenant = Address::generate(&env);
        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30,
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30),
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: Some(registry_address.clone()),
            asset_id: Some(asset_id),
        };

        // Should fail with AssetRegistryInvalid error
        let result = lease_client.try_create_lease_instance(&lease_id, &landlord, &params);
        assert_eq!(result.error(), Some(Ok(LeaseError::AssetRegistryInvalid)));
    }

    #[test]
    fn test_malicious_registry_spoofing_protection() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy malicious registry that spoofs ownership
        let malicious_registry_address = env.register_contract(None, MockRWARegistry);
        let malicious_registry_client = MockRWARegistryClient::new(&env, &malicious_registry_address);
        
        let admin = Address::generate(&env);
        malicious_registry_client.new(&admin);

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        lease_client.initialize(&admin);

        // Whitelist the malicious registry (simulating a compromised admin)
        lease_client.add_whitelisted_registry(&admin, &malicious_registry_address);

        // Don't actually mint any asset - the malicious registry will spoof ownership
        let landlord = Address::generate(&env);
        let asset_id = 12345u128;

        // Try to create lease - should fail because asset doesn't exist
        let tenant = Address::generate(&env);
        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30,
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30),
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: Some(malicious_registry_address.clone()),
            asset_id: Some(asset_id),
        };

        // Should fail because asset doesn't exist in registry
        let result = lease_client.try_create_lease_instance(&lease_id, &landlord, &params);
        assert_eq!(result.error(), Some(Ok(LeaseError::AssetNotOwned)));
    }

    #[test]
    fn test_asset_thawed_on_lease_termination() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy mock RWA registry
        let registry_address = env.register_contract(None, MockRWARegistry);
        let registry_client = MockRWARegistryClient::new(&env, &registry_address);
        
        let admin = Address::generate(&env);
        registry_client.new(&admin);

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        lease_client.initialize(&admin);
        lease_client.add_whitelisted_registry(&admin, &registry_address);

        // Mint asset and create lease
        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        let asset_id = 12345u128;
        let metadata_uri = String::from_str(&env, "ipfs://QmTest");
        registry_client.mint_asset(&asset_id, &landlord, &metadata_uri);

        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30,
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30),
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: Some(registry_address.clone()),
            asset_id: Some(asset_id),
        };

        lease_client.create_lease_instance(&lease_id, &landlord, &params);

        // Verify asset is frozen
        assert!(registry_client.is_asset_frozen(&asset_id));

        // Advance time to end lease
        env.ledger().set_timestamp(env.ledger().timestamp() + 86400 * 31);

        // Settle deposit first
        lease_client.settle_deposit(&lease_id, &tenant, &1000, &0, &0);

        // Terminate lease
        lease_client.terminate_lease(&lease_id, &landlord);

        // Verify asset is thawed
        assert!(!registry_client.is_asset_frozen(&asset_id));
    }

    #[test]
    fn test_lease_creation_without_rwa_asset() {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy lease contract
        let lease_contract_address = env.register_contract(None, LeaseContract);
        let lease_client = LeaseContractClient::new(&env, &lease_contract_address);

        let admin = Address::generate(&env);
        lease_client.initialize(&admin);

        // Create lease without RWA asset (should work normally)
        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        let lease_id = 1u64;
        let params = CreateLeaseParams {
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 86400 * 30,
            property_uri: String::from_str(&env, "test_property"),
            payment_token: Address::generate(&env),
            arbitrators: Vec::new(&env),
            rent_per_sec: 1000 / (86400 * 30),
            grace_period_end: env.ledger().timestamp() + 86400 * 35,
            late_fee_flat: 50,
            late_fee_per_sec: 10,
            equity_percentage_bps: 0,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            yield_delegation_enabled: false,
            deposit_asset: None,
            dex_contract: None,
            max_slippage_bps: 500,
            swap_path: Vec::new(&env),
            asset_registry_address: None,
            asset_id: None,
        };

        // Should succeed without RWA verification
        lease_client.create_lease_instance(&lease_id, &landlord, &params);

        // Verify lease was created without RWA fields
        let lease = lease_client.get_lease_instance(&lease_id);
        assert_eq!(lease.asset_registry_address, None);
        assert_eq!(lease.asset_id, None);
        assert!(!lease.asset_ownership_verified);
        assert!(lease.asset_freeze_proof.is_none());
    }
}
