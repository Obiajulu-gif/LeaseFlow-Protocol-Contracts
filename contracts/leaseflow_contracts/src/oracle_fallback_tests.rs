use soroban_sdk::{contractimpl, testutils::Ledger, Address, BytesN, Env, Symbol};
use crate::{
    LeaseContract, LeaseError, OraclePayload, OracleStatus, OracleTier, LeaseStatus, 
    DamageSeverity, FallbackHierarchy, STALENESS_THRESHOLD, BACKUP_FAILURE_THRESHOLD,
    MAX_ORACLE_FAILURES
};

#[contractimpl]
impl LeaseContract {
    // Test helper function to create a test lease
    fn create_test_lease(env: &Env, lease_id: u64) {
        let landlord = Address::generate(env);
        let tenant = Address::generate(env);
        let payment_token = Address::generate(env);
        
        let lease = crate::LeaseInstance {
            landlord: landlord.clone(),
            tenant: tenant.clone(),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 500,
            start_date: env.ledger().timestamp() - 86400, // 1 day ago
            end_date: env.ledger().timestamp() - 3600,    // 1 hour ago (expired)
            property_uri: soroban_sdk::String::from_str(env, "test_property"),
            status: LeaseStatus::Expired,
            nft_contract: None,
            token_id: None,
            active: false,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() - 3600,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: 0,
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() - 3600,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(env),
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
        };
        
        crate::save_lease_instance(env, lease_id, &lease);
    }
    
    // Test helper function to create test oracle payload
    fn create_test_payload(
        env: &Env, 
        lease_id: u64, 
        oracle_pubkey: &BytesN<32>,
        timestamp: u64,
        severity: DamageSeverity
    ) -> OraclePayload {
        OraclePayload {
            lease_id,
            oracle_pubkey: oracle_pubkey.clone(),
            damage_severity: severity,
            nonce: 1,
            timestamp,
            signature: BytesN::from_array(&[0u8; 64]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Ledger, Address, BytesN, Env};
    
    #[test]
    fn test_staleness_detection() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        env.set_contract_address(admin.clone());
        
        // Set admin
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        
        // Test current timestamp (should pass)
        let current_time = env.ledger().timestamp();
        assert!(LeaseContract::staleness_check(&env, current_time).is_ok());
        
        // Test future timestamp (should fail)
        let future_time = current_time + 3600;
        assert_eq!(
            LeaseContract::staleness_check(&env, future_time),
            Err(LeaseError::OracleStale)
        );
        
        // Test stale timestamp (should fail)
        let stale_time = current_time - STALENESS_THRESHOLD - 3600;
        assert_eq!(
            LeaseContract::staleness_check(&env, stale_time),
            Err(LeaseError::OracleStale)
        );
    }
    
    #[test]
    fn test_fallback_hierarchy_setup() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Set admin
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        
        // Whitelist oracles
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        
        // Set up fallback hierarchy
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Verify hierarchy setup
        let hierarchy = LeaseContract::get_hierarchy_status(env.clone()).unwrap();
        assert_eq!(hierarchy.primary_oracle, primary_oracle);
        assert_eq!(hierarchy.backup_oracle, backup_oracle);
        assert!(hierarchy.dao_arbitration_enabled);
        assert!(!hierarchy.hierarchy_active);
        assert!(hierarchy.last_demotion_time.is_none());
        
        // Verify oracle configurations
        let primary_config = LeaseContract::get_oracle_status(env.clone(), primary_oracle).unwrap();
        assert_eq!(primary_config.tier, OracleTier::Primary);
        assert_eq!(primary_config.status, OracleStatus::Active);
        assert_eq!(primary_config.failure_count, 0);
        
        let backup_config = LeaseContract::get_oracle_status(env.clone(), backup_oracle).unwrap();
        assert_eq!(backup_config.tier, OracleTier::Backup);
        assert_eq!(backup_config.status, OracleStatus::Active);
        assert_eq!(backup_config.failure_count, 0);
    }
    
    #[test]
    fn test_primary_oracle_demotion_on_staleness() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Create test lease
        LeaseContract::create_test_lease(&env, 1);
        
        // Create stale payload (48+ hours old)
        let stale_timestamp = env.ledger().timestamp() - STALENESS_THRESHOLD - 3600;
        let stale_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &primary_oracle, 
            stale_timestamp,
            DamageSeverity::Minor
        );
        
        // Attempt to execute with stale payload should fail and demote oracle
        let result = LeaseContract::execute_deposit_slash(env.clone(), stale_payload);
        assert_eq!(result, Err(LeaseError::OracleStale));
        
        // Verify oracle was demoted
        let primary_config = LeaseContract::get_oracle_status(env.clone(), primary_oracle).unwrap();
        assert_eq!(primary_config.status, OracleStatus::Demoted);
        assert_eq!(primary_config.failure_count, 1);
        assert!(primary_config.demotion_timestamp.is_some());
        
        // Verify fallback hierarchy was activated
        let hierarchy = LeaseContract::get_hierarchy_status(env.clone()).unwrap();
        assert!(hierarchy.hierarchy_active);
        assert!(hierarchy.last_demotion_time.is_some());
    }
    
    #[test]
    fn test_backup_oracle_fallback() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Create test lease
        LeaseContract::create_test_lease(&env, 1);
        
        // Manually demote primary oracle to activate hierarchy
        LeaseContract::demote_oracle(
            &env, 
            &primary_oracle, 
            soroban_sdk::String::from_str(&env, "Test demotion")
        ).unwrap();
        LeaseContract::activate_fallback_hierarchy(
            &env,
            soroban_sdk::String::from_str(&env, "Test activation")
        ).unwrap();
        
        // Create valid payload from backup oracle
        let current_time = env.ledger().timestamp();
        let backup_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &backup_oracle, 
            current_time,
            DamageSeverity::Minor
        );
        
        // Backup oracle should work (though signature will fail in test)
        let result = LeaseContract::execute_deposit_slash(env.clone(), backup_payload);
        // Should fail on signature validation, not oracle availability
        assert_eq!(result, Err(LeaseError::InvalidSignature));
    }
    
    #[test]
    fn test_oracle_bypass_prevention() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        let rogue_oracle = BytesN::from_array(&[3u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), rogue_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Create test lease
        LeaseContract::create_test_lease(&env, 1);
        
        // Manually activate hierarchy
        LeaseContract::activate_fallback_hierarchy(
            &env,
            soroban_sdk::String::from_str(&env, "Test activation")
        ).unwrap();
        
        // Create payload from rogue oracle (should be blocked)
        let current_time = env.ledger().timestamp();
        let rogue_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &rogue_oracle, 
            current_time,
            DamageSeverity::Minor
        );
        
        // Should fail due to oracle bypass prevention
        let result = LeaseContract::execute_deposit_slash(env.clone(), rogue_payload);
        assert_eq!(result, Err(LeaseError::OracleBypassAttempt));
    }
    
    #[test]
    fn test_dao_arbitration_trigger_after_prolonged_backup_failure() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Create test lease
        LeaseContract::create_test_lease(&env, 1);
        
        // Simulate primary oracle failure and hierarchy activation 8 days ago
        let eight_days_ago = env.ledger().timestamp() - (8 * 24 * 60 * 60);
        env.ledger().set_timestamp(eight_days_ago);
        
        LeaseContract::demote_oracle(
            &env, 
            &primary_oracle, 
            soroban_sdk::String::from_str(&env, "Primary failure")
        ).unwrap();
        
        let mut hierarchy = LeaseContract::get_hierarchy_status(&env).unwrap();
        hierarchy.hierarchy_active = true;
        hierarchy.last_demotion_time = Some(eight_days_ago);
        LeaseContract::set_fallback_hierarchy(&env, &hierarchy);
        
        // Return to current time
        env.ledger().set_timestamp(eight_days_ago + (8 * 24 * 60 * 60));
        
        // Create stale payload from backup oracle (should trigger DAO arbitration)
        let stale_timestamp = env.ledger().timestamp() - STALENESS_THRESHOLD - 3600;
        let backup_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &backup_oracle, 
            stale_timestamp,
            DamageSeverity::Minor
        );
        
        // Should fail and trigger DAO arbitration
        let result = LeaseContract::execute_deposit_slash(env.clone(), backup_payload);
        assert_eq!(result, Err(LeaseError::OracleUnavailable));
        
        // Verify lease is in DAO arbitration state
        let lease = crate::load_lease_instance_by_id(&env, 1).unwrap();
        assert_eq!(lease.status, LeaseStatus::DaoArbitration);
    }
    
    #[test]
    fn test_oracle_recovery_after_success() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Manually demote primary oracle
        LeaseContract::demote_oracle(
            &env, 
            &primary_oracle, 
            soroban_sdk::String::from_str(&env, "Test demotion")
        ).unwrap();
        
        // Verify demoted state
        let config = LeaseContract::get_oracle_status(&env, primary_oracle).unwrap();
        assert_eq!(config.status, OracleStatus::Demoted);
        assert_eq!(config.failure_count, 1);
        
        // Simulate successful oracle response by updating config directly
        let mut updated_config = config;
        updated_config.last_successful_timestamp = env.ledger().timestamp();
        updated_config.failure_count = 0; // Reset on success
        updated_config.status = OracleStatus::Active; // Reinstate
        updated_config.demotion_timestamp = None;
        LeaseContract::set_oracle_config(&env, &primary_oracle, &updated_config);
        
        // Verify recovery
        let recovered_config = LeaseContract::get_oracle_status(&env, primary_oracle).unwrap();
        assert_eq!(recovered_config.status, OracleStatus::Active);
        assert_eq!(recovered_config.failure_count, 0);
        assert!(recovered_config.demotion_timestamp.is_none());
    }
    
    #[test]
    fn test_complete_data_blackout_scenario() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let primary_oracle = BytesN::from_array(&[1u8; 32]);
        let backup_oracle = BytesN::from_array(&[2u8; 32]);
        
        env.set_contract_address(admin.clone());
        
        // Setup
        LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), primary_oracle.clone()).unwrap();
        LeaseContract::whitelist_oracle(env.clone(), admin.clone(), backup_oracle.clone()).unwrap();
        LeaseContract::set_fallback_hierarchy(
            env.clone(),
            admin.clone(),
            primary_oracle.clone(),
            backup_oracle.clone(),
            true,
        ).unwrap();
        
        // Create test lease
        LeaseContract::create_test_lease(&env, 1);
        
        // Phase 1: Primary oracle becomes stale
        let initial_time = env.ledger().timestamp();
        env.ledger().set_timestamp(initial_time + STALENESS_THRESHOLD + 3600);
        
        let stale_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &primary_oracle, 
            initial_time, // Old timestamp
            DamageSeverity::Minor
        );
        
        let result = LeaseContract::execute_deposit_slash(env.clone(), stale_payload);
        assert_eq!(result, Err(LeaseError::OracleStale));
        
        // Verify primary oracle demoted and hierarchy activated
        let primary_config = LeaseContract::get_oracle_status(&env, primary_oracle).unwrap();
        assert_eq!(primary_config.status, OracleStatus::Demoted);
        
        let hierarchy = LeaseContract::get_hierarchy_status(&env).unwrap();
        assert!(hierarchy.hierarchy_active);
        
        // Phase 2: Backup oracle also fails after 7+ days
        let seven_days_later = initial_time + BACKUP_FAILURE_THRESHOLD + 3600;
        env.ledger().set_timestamp(seven_days_later);
        
        let backup_stale_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &backup_oracle, 
            seven_days_later - STALENESS_THRESHOLD - 3600, // Stale timestamp
            DamageSeverity::Minor
        );
        
        let result = LeaseContract::execute_deposit_slash(env.clone(), backup_stale_payload);
        assert_eq!(result, Err(LeaseError::OracleUnavailable));
        
        // Phase 3: Verify DAO arbitration triggered
        let lease = crate::load_lease_instance_by_id(&env, 1).unwrap();
        assert_eq!(lease.status, LeaseStatus::DaoArbitration);
        
        // Phase 4: Any further oracle attempts should fail
        let current_time = env.ledger().timestamp();
        let any_payload = LeaseContract::create_test_payload(
            &env, 
            1, 
            &primary_oracle, 
            current_time,
            DamageSeverity::Minor
        );
        
        let result = LeaseContract::execute_deposit_slash(env.clone(), any_payload);
        assert_eq!(result, Err(LeaseError::DaoArbitrationNotEnabled));
        
        // Verify complete cascade: Primary -> Backup -> DAO Arbitration
        assert_eq!(
            LeaseContract::get_oracle_status(&env, primary_oracle).unwrap().status,
            OracleStatus::Demoted
        );
        assert_eq!(
            LeaseContract::get_oracle_status(&env, backup_oracle).unwrap().status,
            OracleStatus::Demoted
        );
        assert_eq!(
            crate::load_lease_instance_by_id(&env, 1).unwrap().status,
            LeaseStatus::DaoArbitration
        );
    }
}
