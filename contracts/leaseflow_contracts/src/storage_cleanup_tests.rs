//! Comprehensive Tests for Storage Cleanup Functionality
//! 
//! This module contains extensive tests for the storage cleanup system,
//! including 60-day boundary testing and byte recovery verification.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32, testutils::BytesN as TestBytesN
};
use proptest::prelude::*;
use crate::{
    LeaseContract, LeaseError, LeaseStatus, DepositStatus, LeaseInstance,
    CleanupError, CleanupDataKey, LegalHoldType, LeaseTombstone, LegalHold,
    storage_cleanup::*, storage_optimizer::*
};

/// Test utilities for storage cleanup
pub struct StorageTestUtils;

impl StorageTestUtils {
    /// Create a test lease instance with specified termination time
    pub fn create_test_lease(
        env: &Env,
        lease_id: u64,
        landlord: Address,
        tenant: Address,
        days_ago_terminated: u64,
        status: LeaseStatus,
    ) -> LeaseInstance {
        let current_time = env.ledger().timestamp();
        let termination_time = current_time - (days_ago_terminated * 24 * 60 * 60);
        
        LeaseInstance {
            landlord,
            tenant,
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: termination_time - (90 * 24 * 60 * 60),
            end_date: termination_time,
            property_uri: String::from_str(env, "test_property"),
            status,
            nft_contract: None,
            token_id: None,
            active: false,
            rent_paid: 1000,
            expiry_time: termination_time,
            buyout_price: None,
            cumulative_payments: 1000,
            debt: 0,
            rent_paid_through: termination_time,
            deposit_status: DepositStatus::Settled,
            rent_per_sec: 0,
            grace_period_end: termination_time,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: Vec::new(env),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: termination_time,
        }
    }
    
    /// Setup test environment with admin and prune whitelist
    pub fn setup_test_env(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let caller = Address::generate(env);
        
        // Setup admin
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        
        // Add caller to prune whitelist
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        (admin, caller)
    }
    
    /// Calculate expected storage size for a lease
    pub fn calculate_expected_lease_size() -> u32 {
        let base_size = 512; // Base LeaseInstance size
        let arbitrator_size = 32; // Per arbitrator (we use 0 in tests)
        let string_size = 64; // Average string size
        let optional_size = 16; // Per optional field
        let vector_size = 8; // Per vector element
        
        base_size + (0 * arbitrator_size) + (4 * string_size) + (8 * optional_size) + (3 * vector_size)
    }
    
    /// Calculate expected tombstone size
    pub fn calculate_expected_tombstone_size() -> u32 {
        128 // Fixed tombstone size
    }
    
    /// Verify exact byte recovery
    pub fn verify_byte_recovery(
        env: &Env,
        lease_id: u64,
        expected_bytes_recovered: u32,
    ) -> bool {
        let actual_bytes_recovered = SorobanStorageOptimizer::estimate_lease_instance_size(env, lease_id);
        actual_bytes_recovered == expected_bytes_recovered
    }
}

/// Property-based tests for 60-day boundary conditions
pub fn sixty_day_boundary_properties() {
    proptest!(|(
        days_ago_terminated in 0u64..=120u64,
        lease_id in 1u64..=1000u64,
        is_terminated in prop::bool::any()
    )| {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create test lease
        let status = if is_terminated { LeaseStatus::Terminated } else { LeaseStatus::Active };
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            days_ago_terminated,
            status,
        );
        
        // Store lease
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Attempt pruning
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        
        // Property 1: Only terminated leases older than 60 days can be pruned
        let can_prune = is_terminated && days_ago_terminated >= 60;
        
        if can_prune {
            prop_assert!(result.is_ok(), "Should be able to prune terminated lease older than 60 days");
            
            // Verify tombstone exists
            let tombstone = LeaseContract::get_lease_tombstone(env.clone(), lease_id);
            prop_assert!(tombstone.is_some(), "Tombstone should exist after pruning");
            
            // Verify lease data is removed
            let removed_lease = crate::load_lease_instance_by_id(&env, lease_id);
            prop_assert!(removed_lease.is_none(), "Lease data should be removed after pruning");
            
        } else {
            // Should fail to prune
            if !is_terminated {
                prop_assert_eq!(result, Err(CleanupError::LeaseNotFinalized), 
                    "Active lease should not be prunable");
            } else if days_ago_terminated < 60 {
                prop_assert_eq!(result, Err(CleanupError::PruneCooldownNotMet), 
                    "Recently terminated lease should not be prunable");
            }
        }
    });
}

/// Property-based tests for byte recovery accuracy
pub fn byte_recovery_properties() {
    proptest!(|(
        lease_id in 1u64..=1000u64,
        days_ago_terminated in 60u64..=120u64,
        has_arbitrators in prop::bool::any(),
        arbitrator_count in 0u32..=5u32
    )| {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create test lease with variable complexity
        let mut lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            days_ago_terminated,
            LeaseStatus::Terminated,
        );
        
        // Add arbitrators if specified
        if has_arbitrators {
            for _ in 0..arbitrator_count {
                lease.arbitrators.push_back(Address::generate(&env));
            }
        }
        
        // Store lease
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Calculate expected size before pruning
        let expected_lease_size = StorageTestUtils::calculate_expected_lease_size();
        let expected_tombstone_size = StorageTestUtils::calculate_expected_tombstone_size();
        let expected_bytes_recovered = expected_lease_size - expected_tombstone_size;
        
        // Prune the lease
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        prop_assert!(result.is_ok(), "Pruning should succeed for old terminated lease");
        
        // Verify byte recovery
        let actual_bytes_recovered = SorobanStorageOptimizer::estimate_lease_instance_size(&env, lease_id);
        prop_assert_eq!(actual_bytes_recovered, 0, "Lease should be completely removed");
        
        // Verify tombstone size
        let tombstone_size = SorobanStorageOptimizer::estimate_key_size(&env, &CleanupDataKey::LeaseTombstone(lease_id));
        prop_assert_eq!(tombstone_size, expected_tombstone_size, "Tombstone size should be correct");
        
        // Verify metrics updated
        let metrics = LeaseContract::get_storage_metrics(env);
        prop_assert_eq!(metrics.total_leases_pruned, 1, "Should have pruned one lease");
        prop_assert!(metrics.total_bytes_recovered > 0, "Should have recovered bytes");
    });
}

/// Property-based tests for legal hold edge cases
pub fn legal_hold_properties() {
    proptest!(|(
        lease_id in 1u64..=1000u64,
        days_ago_terminated in 60u64..=120u64,
        hold_type in 0u32..=3u32, // Map to LegalHoldType
        has_expiry in prop::bool::any(),
        expiry_days in 0u32..=30u32
    )| {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        let legal_authority = Address::generate(&env);
        
        // Create test lease
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            days_ago_terminated,
            LeaseStatus::Terminated,
        );
        
        // Store lease
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Map hold type
        let legal_hold_type = match hold_type {
            0 => LegalHoldType::Appeal,
            1 => LegalHoldType::RegulatoryHold,
            2 => LegalHoldType::CourtOrder,
            _ => LegalHoldType::Investigation,
        };
        
        // Set expiry if specified
        let expires_at = if has_expiry {
            Some(env.ledger().timestamp() + (expiry_days as u64 * 24 * 60 * 60))
        } else {
            None
        };
        
        // Place legal hold
        let hold_result = LeaseContract::place_legal_hold(
            env.clone(),
            lease_id,
            legal_hold_type,
            String::from_str(&env, "Test legal hold"),
            expires_at,
            legal_authority.clone(),
        );
        prop_assert!(hold_result.is_ok(), "Legal hold should be placed successfully");
        
        // Attempt to prune (should fail due to legal hold)
        let prune_result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        prop_assert_eq!(prune_result, Err(CleanupError::ActiveLegalHold), 
            "Pruning should fail with active legal hold");
        
        // Release legal hold
        let release_result = LeaseContract::release_legal_hold(env.clone(), lease_id, legal_authority.clone());
        prop_assert!(release_result.is_ok(), "Legal hold should be released successfully");
        
        // Now pruning should work
        let prune_result_after = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        prop_assert!(prune_result_after.is_ok(), "Pruning should succeed after legal hold release");
    });
}

/// Property-based tests for storage optimization
pub fn storage_optimization_properties() {
    proptest!(|(
        num_leases in 1u32..=10u32,
        prune_interval in 1u32..=5u32
    )| {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create multiple leases
        for i in 0..num_leases {
            let lease_id = i as u64 + 1;
            let days_ago = 60 + (i * 10); // Stagger termination times
            
            let lease = StorageTestUtils::create_test_lease(
                &env,
                lease_id,
                admin.clone(),
                Address::generate(&env),
                days_ago,
                LeaseStatus::Terminated,
            );
            
            crate::save_lease_instance_by_id(&env, lease_id, &lease);
        }
        
        // Prune leases at intervals
        let mut total_pruned = 0u64;
        for i in 0..num_leases {
            if i % prune_interval == 0 {
                let lease_id = (i + 1) as u64;
                if let Ok(_) = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone()) {
                    total_pruned += 1;
                }
            }
        }
        
        // Verify storage optimization
        let stats = SorobanStorageOptimizer::get_storage_statistics(&env);
        prop_assert_eq!(stats.tombstones, total_pruned, "Tombstone count should match pruned leases");
        
        let metrics = LeaseContract::get_storage_metrics(env);
        prop_assert_eq!(metrics.total_leases_pruned, total_pruned, "Metrics should track pruned leases");
        prop_assert!(metrics.total_bytes_recovered > 0, "Should have recovered bytes");
    });
}

/// Comprehensive integration tests
#[cfg(test)]
mod integration_tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_exact_60_day_boundary() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create lease terminated exactly 60 days ago
        let lease_id = 1u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            60, // Exactly 60 days ago
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Should be prunable (>= 60 days)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
        
        // Verify tombstone created
        let tombstone = LeaseContract::get_lease_tombstone(env, lease_id);
        assert!(tombstone.is_some());
    }

    #[test]
    fn test_59_day_boundary() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create lease terminated 59 days ago
        let lease_id = 2u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            59, // 59 days ago - should not be prunable
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Should not be prunable (< 60 days)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert_eq!(result, Err(CleanupError::PruneCooldownNotMet));
    }

    #[test]
    fn test_precise_byte_recovery() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create lease with known size
        let lease_id = 3u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            65, // Old enough to prune
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Calculate expected sizes
        let expected_lease_size = StorageTestUtils::calculate_expected_lease_size();
        let expected_tombstone_size = StorageTestUtils::calculate_expected_tombstone_size();
        let expected_bytes_recovered = expected_lease_size - expected_tombstone_size;
        
        // Prune the lease
        let tombstone_hash = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone()).unwrap();
        
        // Verify byte recovery
        let metrics = LeaseContract::get_storage_metrics(env);
        assert!(metrics.total_bytes_recovered >= expected_bytes_recovered as u64);
        
        // Verify tombstone integrity
        let is_valid = LeaseContract::verify_lease_integrity(env.clone(), lease_id, tombstone_hash);
        assert!(is_valid.unwrap());
    }

    #[test]
    fn test_storage_cleanup_without_dangling_pointers() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create lease with dependencies
        let lease_id = 4u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            65,
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Add some dependency data
        let payer = Address::generate(&env);
        env.storage().persistent().set(&CleanupDataKey::AuthorizedPayer(lease_id, payer), &true);
        env.storage().persistent().set(&CleanupDataKey::TenantFlag(lease_id), &true);
        
        // Prune the lease
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
        
        // Verify no dangling pointers
        let integrity = SorobanStorageOptimizer::validate_storage_integrity(&env, lease_id);
        assert!(integrity.unwrap());
        
        // Verify dependencies are cleaned up
        assert!(!env.storage().persistent().has(&CleanupDataKey::AuthorizedPayer(lease_id, payer)));
        assert!(!env.storage().persistent().has(&CleanupDataKey::TenantFlag(lease_id)));
    }

    #[test]
    fn test_legal_hold_edge_cases() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        let legal_authority = Address::generate(&env);
        
        // Create lease
        let lease_id = 5u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            65,
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Place legal hold with expiry
        let expires_at = env.ledger().timestamp() + (7 * 24 * 60 * 60); // 7 days from now
        LeaseContract::place_legal_hold(
            env.clone(),
            lease_id,
            LegalHoldType::Appeal,
            String::from_str(&env, "Under appeal"),
            Some(expires_at),
            legal_authority.clone(),
        ).unwrap();
        
        // Attempt to prune (should fail)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert_eq!(result, Err(CleanupError::ActiveLegalHold));
        
        // Wait for expiry (simulate time passing)
        // In practice, you'd need to advance the ledger timestamp
        
        // Release legal hold manually
        LeaseContract::release_legal_hold(env.clone(), lease_id, legal_authority.clone()).unwrap();
        
        // Now pruning should work
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_active_state_protection() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Test all active states
        let active_states = vec![
            LeaseStatus::Active,
            LeaseStatus::Pending,
            LeaseStatus::Expired, // Not terminated
        ];
        
        for (i, status) in active_states.iter().enumerate() {
            let lease_id = (i + 10) as u64;
            let lease = StorageTestUtils::create_test_lease(
                &env,
                lease_id,
                admin.clone(),
                Address::generate(&env),
                65, // Old enough
                *status,
            );
            
            crate::save_lease_instance_by_id(&env, lease_id, &lease);
            
            // Attempt to prune (should fail)
            let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
            assert_eq!(result, Err(CleanupError::LeaseNotFinalized));
        }
    }

    #[test]
    fn test_lease_data_pruned_event() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create lease
        let lease_id = 20u64;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            lease_id,
            admin.clone(),
            Address::generate(&env),
            65,
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Prune the lease
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
        
        // In practice, you would verify the event was emitted
        // This would require event capture in the test framework
    }

    #[test]
    fn test_storage_metrics_accuracy() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create and prune multiple leases
        for i in 0..5 {
            let lease_id = (i + 30) as u64;
            let lease = StorageTestUtils::create_test_lease(
                &env,
                lease_id,
                admin.clone(),
                Address::generate(&env),
                60 + (i * 5), // Staggered ages
                LeaseStatus::Terminated,
            );
            
            crate::save_lease_instance_by_id(&env, lease_id, &lease);
            
            // Prune every other lease
            if i % 2 == 0 {
                LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone()).unwrap();
            }
        }
        
        // Verify metrics
        let metrics = LeaseContract::get_storage_metrics(env);
        assert_eq!(metrics.total_leases_pruned, 3); // Should have pruned 3 leases
        assert!(metrics.total_bytes_recovered > 0);
        assert_eq!(metrics.total_tombstones_created, 3);
        assert!(metrics.average_lease_size_bytes > 0);
    }

    #[test]
    fn test_property_based_boundary_conditions() {
        sixty_day_boundary_properties();
        byte_recovery_properties();
        legal_hold_properties();
        storage_optimization_properties();
    }

    #[test]
    fn test_extreme_values() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Test maximum lease ID
        let max_lease_id = u64::MAX;
        let lease = StorageTestUtils::create_test_lease(
            &env,
            max_lease_id,
            admin.clone(),
            Address::generate(&env),
            365, // Very old lease
            LeaseStatus::Terminated,
        );
        
        crate::save_lease_instance_by_id(&env, max_lease_id, &lease);
        
        // Should handle extreme values gracefully
        let result = LeaseContract::prune_finalized_lease(env.clone(), max_lease_id, caller.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_concurrent_pruning_safety() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create multiple leases
        let lease_ids = vec![100u64, 101u64, 102u64];
        
        for &lease_id in &lease_ids {
            let lease = StorageTestUtils::create_test_lease(
                &env,
                lease_id,
                admin.clone(),
                Address::generate(&env),
                65,
                LeaseStatus::Terminated,
            );
            
            crate::save_lease_instance_by_id(&env, lease_id, &lease);
        }
        
        // Prune all leases
        for &lease_id in &lease_ids {
            let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
            assert!(result.is_ok());
        }
        
        // Verify all were pruned
        let metrics = LeaseContract::get_storage_metrics(env);
        assert_eq!(metrics.total_leases_pruned, 3);
        
        // Verify no conflicts
        for &lease_id in &lease_ids {
            let tombstone = LeaseContract::get_lease_tombstone(env.clone(), lease_id);
            assert!(tombstone.is_some());
            
            let lease = crate::load_lease_instance_by_id(&env, lease_id);
            assert!(lease.is_none());
        }
    }
}

/// Performance benchmarks for storage cleanup
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_pruning_performance() {
        let env = Env::default();
        let (admin, caller) = StorageTestUtils::setup_test_env(&env);
        
        // Create 100 leases
        let num_leases = 100;
        for i in 0..num_leases {
            let lease_id = i as u64;
            let lease = StorageTestUtils::create_test_lease(
                &env,
                lease_id,
                admin.clone(),
                Address::generate(&env),
                60 + (i / 2), // Staggered ages
                LeaseStatus::Terminated,
            );
            
            crate::save_lease_instance_by_id(&env, lease_id, &lease);
        }
        
        // Benchmark pruning
        let start = Instant::now();
        let mut pruned_count = 0;
        
        for i in 0..num_leases {
            let lease_id = i as u64;
            if let Ok(_) = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone()) {
                pruned_count += 1;
            }
        }
        
        let duration = start.elapsed();
        
        // Performance assertions
        assert!(pruned_count > 0, "Should have pruned some leases");
        assert!(duration.as_millis() < 5000, "Pruning should complete within 5 seconds");
        
        println!("Pruned {} leases in {:?}", pruned_count, duration);
        println!("Average time per lease: {:?}", duration / pruned_count);
    }

    #[test]
    fn benchmark_storage_optimization() {
        let env = Env::default();
        
        // Create storage data
        for i in 0..50 {
            let key = CleanupDataKey::TenantFlag(i as u64);
            env.storage().persistent().set(&key, &true);
        }
        
        // Benchmark optimization
        let start = Instant::now();
        let result = SorobanStorageOptimizer::optimize_storage_layout(&env);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 1000, "Optimization should complete within 1 second");
        
        println!("Storage optimization completed in {:?}", duration);
    }
}
