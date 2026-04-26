use soroban_sdk::{contractimpl, testutils::Ledger, Address, BytesN, Env, Symbol};
use crate::{
    LeaseContract, LeaseError, AssetConditionMetadataPayload, AssetCondition, 
    LeaseStatus, OracleStatus, OracleTier, DamageSeverity, FallbackHierarchy,
    AssetMetadataUpdated, YEAR_IN_LEDGERS
};

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Ledger, Address, BytesN, Env};

    #[test]
    fn test_update_asset_condition_metadata_success() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        
        // Whitelist oracle
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();
        
        // Create test lease
        let lease_id = 1;
        let tenant = Address::generate(&env);
        let landlord = Address::generate(&env);
        let payment_token = Address::generate(&env);
        
        let lease = crate::LeaseInstance {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60, // 30 days
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: payment_token.clone(),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // Create oracle payload for asset condition update
        let payload = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Damaged,
            nonce: 1,
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        // Mock asset registry contract
        let asset_registry_id = env.register_contract(&asset_registry, crate::asset_registry::AssetRegistryClient);
        env.mock_all_auths();
        
        // Update asset condition
        let result = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload,
            asset_registry.clone(),
        );
        
        assert!(result.is_ok());
        
        // Verify asset condition was updated
        let updated_condition = LeaseContract::get_asset_condition(env.clone(), lease_id);
        assert_eq!(updated_condition, AssetCondition::Damaged);
        
        // Verify AssetMetadataUpdated event was emitted
        let events = env.events().all();
        assert!(events.len() > 0);
        
        // Check for AssetMetadataUpdated event
        let mut found_event = false;
        for event in events {
            if let Some(asset_event) = event.clone().try_into_val::<AssetMetadataUpdated>(&env) {
                assert_eq!(asset_event.lease_id, lease_id);
                assert_eq!(asset_event.asset_condition, AssetCondition::Damaged);
                assert_eq!(asset_event.oracle_pubkey, oracle_pubkey);
                assert_eq!(asset_event.previous_condition, AssetCondition::Mint);
                found_event = true;
                break;
            }
        }
        assert!(found_event, "AssetMetadataUpdated event not found");
    }

    #[test]
    fn test_update_asset_condition_unauthorized_oracle() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let unauthorized_oracle = BytesN::from_array(&env, &[2; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin but don't whitelist oracle
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        
        // Create test lease
        let lease_id = 1;
        let lease = crate::LeaseInstance {
            landlord: Address::generate(&env),
            tenant: Address::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: Address::generate(&env),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // Create payload from unauthorized oracle
        let payload = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: unauthorized_oracle,
            asset_condition: AssetCondition::Damaged,
            nonce: 1,
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        // Should fail due to unauthorized oracle
        let result = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload,
            asset_registry,
        );
        
        assert_eq!(result, Err(LeaseError::OracleNotWhitelisted));
    }

    #[test]
    fn test_rate_limiting_prevents_spam() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin and whitelist oracle
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();
        
        // Create test lease
        let lease_id = 1;
        let lease = crate::LeaseInstance {
            landlord: Address::generate(&env),
            tenant: Address::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: Address::generate(&env),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // First update should succeed
        let payload1 = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Worn,
            nonce: 1,
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        let result1 = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload1,
            asset_registry.clone(),
        );
        assert!(result1.is_ok());
        
        // Second update in same hour should fail due to rate limiting
        let payload2 = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Damaged,
            nonce: 2,
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        let result2 = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload2,
            asset_registry,
        );
        assert_eq!(result2, Err(LeaseError::OracleStale)); // Reused error for rate limiting
    }

    #[test]
    fn test_destroyed_condition_triggers_termination() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin and whitelist oracle
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();
        
        // Create active lease
        let lease_id = 1;
        let tenant = Address::generate(&env);
        let landlord = Address::generate(&env);
        let payment_token = Address::generate(&env);
        
        let lease = crate::LeaseInstance {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: payment_token.clone(),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // Create payload for destroyed condition
        let payload = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Destroyed,
            nonce: 1,
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        // Mock asset registry contract
        let asset_registry_id = env.register_contract(&asset_registry, crate::asset_registry::AssetRegistryClient);
        env.mock_all_auths();
        
        // Update asset condition to Destroyed
        let result = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload,
            asset_registry,
        );
        
        assert!(result.is_ok());
        
        // Verify lease was terminated
        let terminated_lease = crate::load_lease_instance_by_id(&env, lease_id);
        assert!(terminated_lease.is_none()); // Should be archived
        
        // Verify asset condition was updated
        let updated_condition = LeaseContract::get_asset_condition(env.clone(), lease_id);
        assert_eq!(updated_condition, AssetCondition::Destroyed);
        
        // Verify lease is in historical records
        let historical_lease = env.storage()
            .persistent()
            .get::<_, crate::HistoricalLease>(&crate::DataKey::HistoricalLease(lease_id));
        assert!(historical_lease.is_some());
        assert_eq!(historical_lease.unwrap().lease.status, LeaseStatus::Terminated);
    }

    #[test]
    fn test_invalid_nonce_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin and whitelist oracle
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();
        
        // Set oracle nonce to 5
        crate::LeaseContract::set_oracle_nonce(&env, &oracle_pubkey, 5);
        
        // Create test lease
        let lease_id = 1;
        let lease = crate::LeaseInstance {
            landlord: Address::generate(&env),
            tenant: Address::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: Address::generate(&env),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // Create payload with nonce <= stored nonce (should fail)
        let payload = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Damaged,
            nonce: 5, // Same as stored nonce
            timestamp: env.ledger().timestamp(),
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        let result = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload,
            asset_registry,
        );
        
        assert_eq!(result, Err(LeaseError::InvalidNonce));
    }

    #[test]
    fn test_stale_timestamp_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);
        let asset_registry = Address::generate(&env);
        
        env.set_contract_address(admin.clone());
        
        // Initialize admin and whitelist oracle
        env.storage().instance().set(&crate::DataKey::Admin, &admin);
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();
        
        // Create test lease
        let lease_id = 1;
        let lease = crate::LeaseInstance {
            landlord: Address::generate(&env),
            tenant: Address::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            property_uri: soroban_sdk::String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 1000 / (30 * 24 * 60 * 60),
            grace_period_end: env.ledger().timestamp() + 30 * 24 * 60 * 60,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(&env),
            maintenance_status: crate::MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
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
            payment_token: Address::generate(&env),
        };
        
        crate::save_lease_instance(&env, lease_id, &lease);
        
        // Create payload with stale timestamp (more than 48 hours old)
        let stale_timestamp = env.ledger().timestamp() - (49 * 60 * 60); // 49 hours ago
        let payload = AssetConditionMetadataPayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            asset_condition: AssetCondition::Damaged,
            nonce: 1,
            timestamp: stale_timestamp,
            signature: BytesN::from_array(&[0u8; 64]),
        };
        
        let result = LeaseContract::update_asset_condition_metadata(
            env.clone(),
            payload,
            asset_registry,
        );
        
        assert_eq!(result, Err(LeaseError::OracleStale));
    }
}
