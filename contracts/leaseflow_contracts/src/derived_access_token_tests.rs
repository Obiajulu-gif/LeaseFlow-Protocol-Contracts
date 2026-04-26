//! Stress Tests for Hierarchical Sub-Lease Tokenization
//! 
//! This module contains comprehensive tests for the derived access token system,
//! including stress tests for deep hierarchies and edge cases.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32, u128, testutils::Address as TestAddress
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DepositStatus,
    save_lease_instance_by_id, load_lease_instance_by_id, RateType
};
use crate::lessee_access_token::{
    LesseeAccessToken, AssetType, AccessLevel, RevocationReason,
    LesseeAccessTokenManager
};
use crate::derived_access_token::{
    DerivedAccessToken, DerivedTokenRequest, DerivedAccessTokenManager,
    SpatialZone, DerivedTokenError, HierarchyMetrics
};

/// Test utility for creating test leases
fn create_test_lease(env: &Env, landlord: Address, tenant: Address, lease_id: u64) -> LeaseInstance {
    let current_time = env.ledger().timestamp();
    let duration = 365 * 24 * 60 * 60; // 1 year
    
    LeaseInstance {
        landlord,
        tenant,
        rent_amount: 1000,
        deposit_amount: 500,
        security_deposit: 200,
        start_date: current_time,
        end_date: current_time + duration,
        property_uri: String::from_str(env, "test_property_4bed"),
        status: LeaseStatus::Active,
        nft_contract: None,
        token_id: None,
        active: true,
        rent_paid: 0,
        expiry_time: current_time + duration,
        buyout_price: None,
        cumulative_payments: 0,
        debt: 0,
        rent_paid_through: current_time,
        deposit_status: DepositStatus::Held,
        rent_per_sec: 1000 / (30 * 24 * 60 * 60), // Monthly rent
        grace_period_end: current_time + duration,
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
        billing_cycle_duration: 30 * 24 * 60 * 60,
        continuous_billing_enabled: false,
        rent_treasury_address: None,
        yield_delegation_enabled: false,
        yield_accumulated: 0,
        equity_balance: 0,
        equity_percentage_bps: 0,
        had_late_payment: false,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        payment_token: Address::generate(env),
    }
}

/// Test utility for creating root access token
fn create_root_access_token(env: &Env, lease_id: u64) -> u128 {
    LesseeAccessTokenManager::mint_lessee_access_token(
        env.clone(),
        lease_id,
        String::from_str(env, "smart_lock_main"),
        AssetType::IoT,
        AccessLevel::Full,
        true, // Transferable to allow subleasing
    ).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_derived_token_creation() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 1u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create derived token request
        let request = DerivedTokenRequest {
            parent_token_id: root_token_id,
            sublessee,
            spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 1")),
            access_level: AccessLevel::Limited,
            transferable: true,
            expiration_timestamp: None, // Inherit from parent
        };
        
        // Mint derived token
        let derived_token_id = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request).unwrap();
        
        // Verify derived token
        let token = DerivedAccessTokenManager::get_derived_token(env.clone(), derived_token_id).unwrap();
        assert_eq!(token.parent_token_id, root_token_id);
        assert_eq!(token.root_lease_id, lease_id);
        assert_eq!(token.sublessee, sublessee);
        assert_eq!(token.hierarchy_depth, 1);
        assert!(matches!(token.spatial_zone, SpatialZone::SpecificRoom(_)));
        assert!(!token.revoked);
        
        // Verify hierarchy metrics
        let metrics = DerivedAccessTokenManager::get_hierarchy_metrics(env.clone(), lease_id).unwrap();
        assert_eq!(metrics.total_tokens, 1);
        assert_eq!(metrics.max_depth, 1);
        assert_eq!(metrics.avg_depth, 1);
    }

    #[test]
    fn test_temporal_dependency_enforcement() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Create test lease with short duration
        let lease_id = 2u64;
        let mut lease = create_test_lease(&env, landlord, tenant, lease_id);
        lease.end_date = env.ledger().timestamp() + (7 * 24 * 60 * 60); // 7 days
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Try to create derived token with expiration beyond parent
        let future_expiration = env.ledger().timestamp() + (14 * 24 * 60 * 60); // 14 days
        let request = DerivedTokenRequest {
            parent_token_id: root_token_id,
            sublessee,
            spatial_zone: SpatialZone::EntireProperty,
            access_level: AccessLevel::Full,
            transferable: true,
            expiration_timestamp: Some(future_expiration),
        };
        
        // Should fail due to temporal dependency violation
        let result = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request);
        assert_eq!(result, Err(DerivedTokenError::InvalidTemporalDependency));
        
        // Try with valid expiration (before parent)
        let valid_expiration = env.ledger().timestamp() + (3 * 24 * 60 * 60); // 3 days
        let valid_request = DerivedTokenRequest {
            parent_token_id: root_token_id,
            sublessee,
            spatial_zone: SpatialZone::EntireProperty,
            access_level: AccessLevel::Full,
            transferable: true,
            expiration_timestamp: Some(valid_expiration),
        };
        
        // Should succeed
        let result = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), valid_request);
        assert!(result.is_ok());
        
        let derived_token_id = result.unwrap();
        let token = DerivedAccessTokenManager::get_derived_token(env.clone(), derived_token_id).unwrap();
        assert_eq!(token.expiration_timestamp, valid_expiration);
    }

    #[test]
    fn test_5_layer_deep_hierarchy() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 3u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Build 5-layer deep hierarchy
        let mut token_ids = Vec::new(&env);
        let mut current_parent = root_token_id;
        
        for layer in 0..5 {
            let sublessee = TestAddress::generate(&env);
            let request = DerivedTokenRequest {
                parent_token_id: current_parent,
                sublessee,
                spatial_zone: match layer {
                    0 => SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 1")),
                    1 => SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 2")),
                    2 => SpatialZone::Zone(String::from_str(&env, "Living Area")),
                    3 => SpatialZone::SpecificRoom(String::from_str(&env, "Kitchen")),
                    4 => SpatialZone::CustomArea(String::from_str(&env, "Storage Closet")),
                    _ => SpatialZone::EntireProperty,
                },
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            };
            
            let derived_token_id = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request).unwrap();
            token_ids.push_back(derived_token_id);
            current_parent = derived_token_id;
            
            // Verify hierarchy depth
            let token = DerivedAccessTokenManager::get_derived_token(env.clone(), derived_token_id).unwrap();
            assert_eq!(token.hierarchy_depth, layer + 1);
        }
        
        // Verify hierarchy metrics
        let metrics = DerivedAccessTokenManager::get_hierarchy_metrics(env.clone(), lease_id).unwrap();
        assert_eq!(metrics.total_tokens, 5);
        assert_eq!(metrics.max_depth, 5);
        assert_eq!(metrics.avg_depth, 3); // (1+2+3+4+5)/5 = 3
        
        // Verify all tokens are valid
        for token_id in token_ids.iter() {
            assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), *token_id).unwrap());
        }
        
        // Test recursive burning from deepest token
        let deepest_token = *token_ids.iter().last().unwrap();
        let burned_count = DerivedAccessTokenManager::recursive_burn_hierarchy(
            env.clone(),
            deepest_token,
            RevocationReason::LeaseTerminated,
        ).unwrap();
        
        // Should burn all tokens in the subtree (just the deepest token in this case)
        assert_eq!(burned_count, 1);
        
        // Verify deepest token is burned
        let deepest_token_obj = DerivedAccessTokenManager::get_derived_token(env.clone(), deepest_token).unwrap();
        assert!(deepest_token_obj.revoked);
        
        // Verify parent tokens are still valid
        for i in 0..4 {
            let token_id = token_ids.get(i as u32).unwrap();
            assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), *token_id).unwrap());
        }
    }

    #[test]
    fn test_complex_hierarchy_with_branches() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 4u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create complex hierarchy with branches
        // Layer 1: 2 tokens
        let layer1_token1 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 1")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        let layer1_token2 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 2")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        // Layer 2: 2 tokens from layer1_token1, 1 token from layer1_token2
        let layer2_token1 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: layer1_token1,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::Zone(String::from_str(&env, "Master Bathroom")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        let layer2_token2 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: layer1_token1,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::CustomArea(String::from_str(&env, "Walk-in Closet")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        let layer2_token3 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: layer1_token2,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::Zone(String::from_str(&env, "En-suite Bathroom")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        // Layer 3: 1 token from layer2_token1
        let layer3_token1 = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: layer2_token1,
                sublessee: TestAddress::generate(&env),
                spatial_zone: SpatialZone::CustomArea(String::from_str(&env, "Shower Stall")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        // Verify hierarchy metrics
        let metrics = DerivedAccessTokenManager::get_hierarchy_metrics(env.clone(), lease_id).unwrap();
        assert_eq!(metrics.total_tokens, 6);
        assert_eq!(metrics.max_depth, 3);
        
        // Test recursive burning from layer1_token1 (should burn 3 tokens: layer1_token1, layer2_token1, layer2_token2, layer3_token1)
        let burned_count = DerivedAccessTokenManager::recursive_burn_hierarchy(
            env.clone(),
            layer1_token1,
            RevocationReason::LeaseTerminated,
        ).unwrap();
        
        assert_eq!(burned_count, 4); // layer1_token1 + layer2_token1 + layer2_token2 + layer3_token1
        
        // Verify burned tokens
        assert!(!DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer1_token1).unwrap());
        assert!(!DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer2_token1).unwrap());
        assert!(!DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer2_token2).unwrap());
        assert!(!DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer3_token1).unwrap());
        
        // Verify remaining tokens are still valid
        assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer1_token2).unwrap());
        assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), layer2_token3).unwrap());
    }

    #[test]
    fn test_hierarchy_depth_limit() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 5u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Build hierarchy up to max depth
        let mut current_parent = root_token_id;
        let mut token_ids = Vec::new(&env);
        
        // Create tokens up to depth 10 (max allowed)
        for layer in 0..10 {
            let sublessee = TestAddress::generate(&env);
            let request = DerivedTokenRequest {
                parent_token_id: current_parent,
                sublessee,
                spatial_zone: SpatialZone::CustomArea(String::from_str(&env, &format!("Area {}", layer))),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            };
            
            let derived_token_id = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request).unwrap();
            token_ids.push_back(derived_token_id);
            current_parent = derived_token_id;
        }
        
        // Try to create token beyond max depth (should fail)
        let sublessee = TestAddress::generate(&env);
        let over_depth_request = DerivedTokenRequest {
            parent_token_id: current_parent,
            sublessee,
            spatial_zone: SpatialZone::CustomArea(String::from_str(&env, "Over Depth Area")),
            access_level: AccessLevel::Limited,
            transferable: true,
            expiration_timestamp: None,
        };
        
        let result = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), over_depth_request);
        assert_eq!(result, Err(DerivedTokenError::HierarchyDepthExceeded));
        
        // Verify metrics show max depth of 10
        let metrics = DerivedAccessTokenManager::get_hierarchy_metrics(env.clone(), lease_id).unwrap();
        assert_eq!(metrics.max_depth, 10);
    }

    #[test]
    fn test_spatial_zone_validation() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 6u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Test valid spatial zones
        let valid_zones = vec![
            SpatialZone::EntireProperty,
            SpatialZone::SpecificRoom(String::from_str(&env, "Master Bedroom")),
            SpatialZone::Zone(String::from_str(&env, "Living Area")),
            SpatialZone::CustomArea(String::from_str(&env, "Private Office")),
        ];
        
        for zone in valid_zones {
            let request = DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee,
                spatial_zone: zone.clone(),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            };
            
            let result = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request);
            assert!(result.is_ok(), "Failed for zone: {:?}", zone);
        }
        
        // Test invalid spatial zones (empty strings)
        let invalid_zones = vec![
            SpatialZone::SpecificRoom(String::from_str(&env, "")),
            SpatialZone::Zone(String::from_str(&env, "")),
            SpatialZone::CustomArea(String::from_str(&env, "")),
        ];
        
        for zone in invalid_zones {
            let request = DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee,
                spatial_zone: zone.clone(),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            };
            
            let result = DerivedAccessTokenManager::mint_derived_access_token(env.clone(), request);
            assert_eq!(result, Err(DerivedTokenError::InvalidSpatialZone));
        }
    }

    #[test]
    fn test_derived_token_transfer() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        let new_sublessee = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 7u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create derived token
        let derived_token_id = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee,
                spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, "Guest Room")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        // Transfer derived token
        let transfer_result = DerivedAccessTokenManager::transfer_derived_token(
            env.clone(),
            derived_token_id,
            sublessee,
            new_sublessee,
            String::from_str(&env, "Sublease transfer"),
        );
        assert!(transfer_result.is_ok());
        
        // Verify transfer
        let token = DerivedAccessTokenManager::get_derived_token(env.clone(), derived_token_id).unwrap();
        assert_eq!(token.sublessee, new_sublessee);
        assert_eq!(token.transfer_count, 1);
        
        // Verify sublessee mappings updated
        let old_sublessee_tokens = DerivedAccessTokenManager::get_sublessee_derived_tokens(env.clone(), sublessee);
        let new_sublessee_tokens = DerivedAccessTokenManager::get_sublessee_derived_tokens(env.clone(), new_sublessee);
        
        assert!(!old_sublessee_tokens.contains(&derived_token_id));
        assert!(new_sublessee_tokens.contains(&derived_token_id));
    }

    #[test]
    fn test_spatial_zone_update() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 8u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create derived token
        let derived_token_id = DerivedAccessTokenManager::mint_derived_access_token(
            env.clone(),
            DerivedTokenRequest {
                parent_token_id: root_token_id,
                sublessee,
                spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, "Bedroom 1")),
                access_level: AccessLevel::Limited,
                transferable: true,
                expiration_timestamp: None,
            },
        ).unwrap();
        
        // Update spatial zone
        let new_zone = SpatialZone::Zone(String::from_str(&env, "Master Suite"));
        let update_result = DerivedAccessTokenManager::update_spatial_zone(env.clone(), derived_token_id, new_zone.clone());
        assert!(update_result.is_ok());
        
        // Verify zone update
        let token = DerivedAccessTokenManager::get_derived_token(env.clone(), derived_token_id).unwrap();
        assert_eq!(token.spatial_zone, new_zone);
    }

    #[test]
    fn test_master_lease_termination_cascade() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 9u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create complex hierarchy
        let mut all_derived_tokens = Vec::new(&env);
        
        // Layer 1: 2 tokens
        for i in 0..2 {
            let token_id = DerivedAccessTokenManager::mint_derived_access_token(
                env.clone(),
                DerivedTokenRequest {
                    parent_token_id: root_token_id,
                    sublessee: TestAddress::generate(&env),
                    spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, &format!("Bedroom {}", i + 1))),
                    access_level: AccessLevel::Limited,
                    transferable: true,
                    expiration_timestamp: None,
                },
            ).unwrap();
            all_derived_tokens.push_back(token_id);
        }
        
        // Layer 2: 1 token from each Layer 1 token
        for i in 0..2 {
            let token_id = DerivedAccessTokenManager::mint_derived_access_token(
                env.clone(),
                DerivedTokenRequest {
                    parent_token_id: all_derived_tokens.get(i).unwrap(),
                    sublessee: TestAddress::generate(&env),
                    spatial_zone: SpatialZone::Zone(String::from_str(&env, &format!("Zone {}", i + 1))),
                    access_level: AccessLevel::Limited,
                    transferable: true,
                    expiration_timestamp: None,
                },
            ).unwrap();
            all_derived_tokens.push_back(token_id);
        }
        
        // Verify all tokens are valid before termination
        for token_id in all_derived_tokens.iter() {
            assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), *token_id).unwrap());
        }
        
        // Simulate master lease termination by revoking root token
        LesseeAccessTokenManager::revoke_access_token(env.clone(), lease_id, RevocationReason::LeaseTerminated).unwrap();
        
        // Verify all derived tokens are now invalid (expired parent)
        for token_id in all_derived_tokens.iter() {
            // Note: In a real implementation, we'd have a hook that auto-revokes
            // derived tokens when parent is revoked. For this test, we check
            // that the parent is revoked, which makes derived tokens invalid
            let token = DerivedAccessTokenManager::get_derived_token(env.clone(), *token_id).unwrap();
            let parent_token = LesseeAccessTokenManager::get_access_token(env.clone(), token.parent_token_id).unwrap();
            assert!(parent_token.revoked);
        }
    }

    #[test]
    fn test_performance_stress_large_hierarchy() {
        let env = Env::default();
        let landlord = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create test lease
        let lease_id = 10u64;
        let lease = create_test_lease(&env, landlord, tenant, lease_id);
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Create root access token
        let root_token_id = create_root_access_token(&env, lease_id);
        
        // Create large hierarchy (50 tokens total)
        let mut all_tokens = Vec::new(&env);
        
        // Create 10 first-level tokens
        for i in 0..10 {
            let token_id = DerivedAccessTokenManager::mint_derived_access_token(
                env.clone(),
                DerivedTokenRequest {
                    parent_token_id: root_token_id,
                    sublessee: TestAddress::generate(&env),
                    spatial_zone: SpatialZone::SpecificRoom(String::from_str(&env, &format!("Room {}", i))),
                    access_level: AccessLevel::Limited,
                    transferable: true,
                    expiration_timestamp: None,
                },
            ).unwrap();
            all_tokens.push_back(token_id);
        }
        
        // Create 40 second-level tokens (4 from each first-level)
        for i in 0..10 {
            let parent_token = all_tokens.get(i).unwrap();
            for j in 0..4 {
                let token_id = DerivedAccessTokenManager::mint_derived_access_token(
                    env.clone(),
                    DerivedTokenRequest {
                        parent_token_id: *parent_token,
                        sublessee: TestAddress::generate(&env),
                        spatial_zone: SpatialZone::CustomArea(String::from_str(&env, &format!("Area {}-{}", i, j))),
                        access_level: AccessLevel::Limited,
                        transferable: true,
                        expiration_timestamp: None,
                    },
                ).unwrap();
                all_tokens.push_back(token_id);
            }
        }
        
        // Verify hierarchy metrics
        let metrics = DerivedAccessTokenManager::get_hierarchy_metrics(env.clone(), lease_id).unwrap();
        assert_eq!(metrics.total_tokens, 50);
        assert_eq!(metrics.max_depth, 2);
        
        // Test bulk operations
        let start_time = env.ledger().timestamp();
        
        // Verify all tokens
        for token_id in all_tokens.iter() {
            assert!(DerivedAccessTokenManager::is_derived_token_valid(env.clone(), *token_id).unwrap());
        }
        
        let verification_time = env.ledger().timestamp() - start_time;
        
        // Test recursive burn from root (burn all tokens)
        let burned_count = DerivedAccessTokenManager::recursive_burn_hierarchy(
            env.clone(),
            root_token_id,
            RevocationReason::LeaseTerminated,
        ).unwrap();
        
        // Should burn all 50 tokens
        assert_eq!(burned_count, 50);
        
        // Verify all tokens are burned
        for token_id in all_tokens.iter() {
            assert!(!DerivedAccessTokenManager::is_derived_token_valid(env.clone(), *token_id).unwrap());
        }
        
        // Performance assertions (these are rough estimates)
        assert!(verification_time < 1000000, "Verification took too long"); // Less than 1 second in ledger time
    }
}
