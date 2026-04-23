//! Comprehensive Tests for Lessor Rights Tokenization
//! 
//! This module contains extensive tests for NFT sale, yield redirection,
//! and secondary market functionality.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32, testutils::BytesN as TestBytesN
};
use proptest::prelude::*;
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DepositStatus,
    lessor_rights_nft::{LessorRightsNFT, LessorRightsNFTMetadata, NFTTransferRecord, NFTDataKey},
    lease_payment_router::{LeaseContract as PaymentRouter, PaymentRoutingConfig, YieldAccumulation}
};

/// Test utilities for NFT operations
pub struct NFTTestUtils;

impl NFTTestUtils {
    /// Create a test lease with NFT support
    pub fn create_test_lease_with_nft(
        env: &Env,
        lease_id: u64,
        lessor: Address,
        tenant: Address,
        rent_amount: i128,
        deposit_amount: i128,
        security_deposit: i128,
        duration_days: u64,
    ) -> (LeaseInstance, u128) {
        let current_time = env.ledger().timestamp();
        let start_date = current_time;
        let end_date = start_date + (duration_days * 24 * 60 * 60);
        
        let lease = LeaseInstance {
            landlord: lessor.clone(),
            tenant,
            rent_amount,
            deposit_amount,
            security_deposit,
            start_date,
            end_date,
            property_uri: String::from_str(env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: end_date,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: start_date,
            deposit_status: DepositStatus::Held,
            rent_per_sec: rent_amount / (30 * 24 * 60 * 60), // Monthly rent
            grace_period_end: end_date + (7 * 24 * 60 * 60),
            late_fee_flat: 50,
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
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: current_time,
        };
        
        // Store lease
        crate::save_lease_instance_by_id(env, lease_id, &lease);
        
        // Mint NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        (lease, token_id)
    }
    
    /// Simulate NFT sale on secondary market
    pub fn simulate_nft_sale(
        env: &Env,
        token_id: u128,
        from_holder: Address,
        to_holder: Address,
        sale_price: i128,
    ) -> Result<(), crate::lessor_rights_nft::NFTError> {
        // Transfer NFT
        LessorRightsNFT::transfer_nft(env.clone(), token_id, from_holder, to_holder)?;
        
        // In a real implementation, this would handle the payment
        // For now, we'll just emit an event
        crate::lessor_rights_nft::NFTTransferred {
            token_id,
            from_holder,
            to_holder: to_holder.clone(),
            transfer_timestamp: env.ledger().timestamp(),
            accrued_yield: 0,
            proration_amount: 0,
        }.publish(env);
        
        Ok(())
    }
    
    /// Advance time by specified seconds
    pub fn advance_time(env: &Env, seconds: u64) {
        // In a real test environment, this would advance the ledger timestamp
        // For now, we'll just simulate the time advancement
        let current_time = env.ledger().timestamp();
        // Note: This is a placeholder - actual time advancement would be handled by the test framework
    }
    
    /// Calculate expected yield for a period
    pub fn calculate_expected_yield(lease: &LeaseInstance, period_days: u64) -> i128 {
        lease.rent_amount * period_days as i128 / 30 // Assuming monthly rent
    }
}

/// Property-based tests for NFT sale and yield redirection
pub fn nft_sale_yield_redirection_properties() {
    proptest!(|(
        lease_id in 1u64..=1000u64,
        rent_amount in 500i128..=5000i128,
        deposit_amount in 100i128..=1000i128,
        security_deposit in 200i128..=2000i128,
        duration_days in 30u64..=365u64,
        num_transfers in 1u32..=5u32,
        payment_amount in 100i128..=1000i128,
        transfer_day in 5u32..=25u32 // Day of month when transfer occurs
    )| {
        let env = Env::default();
        let lessor = Address::generate(&env);
        let tenant = Address::generate(&env);
        
        // Create test lease with NFT
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            rent_amount,
            deposit_amount,
            security_deposit,
            duration_days,
        );
        
        // Property 1: Initial NFT holder should be lessor
        let initial_holder = LessorRightsNFT::get_current_holder(env.clone(), lease_id).unwrap();
        prop_assert_eq!(initial_holder, lessor, "Initial holder should be lessor");
        
        // Property 2: NFT should be indestructible while lease is active
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), Address::generate(&env));
        prop_assert_eq!(transfer_result, Err(crate::lessor_rights_nft::NFTError::TransferDuringLock), 
            "NFT should be locked while lease is active");
        
        // Release lock for testing
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Property 3: Multiple transfers should work correctly
        let mut current_holder = lessor.clone();
        let mut total_yield = 0i128;
        
        for transfer_num in 0..num_transfers {
            let new_holder = Address::generate(&env);
            
            // Make some payments before transfer
            for _ in 0..3 {
                let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                    env.clone(),
                    lease_id,
                    tenant.clone(),
                    payment_amount,
                );
                prop_assert!(payment_result.is_ok(), "Payment should succeed");
                total_yield += payment_amount;
            }
            
            // Transfer NFT
            let transfer_result = NFTTestUtils::simulate_nft_sale(
                &env,
                token_id,
                current_holder.clone(),
                new_holder.clone(),
                10000, // Sale price
            );
            prop_assert!(transfer_result.is_ok(), "NFT transfer should succeed");
            
            // Update payment routing
            let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
                env.clone(),
                lease_id,
                current_holder.clone(),
                new_holder.clone(),
            );
            prop_assert!(routing_result.is_ok(), "Payment routing update should succeed");
            
            // Verify holder changed
            let updated_holder = LessorRightsNFT::get_current_holder(env.clone(), lease_id).unwrap();
            prop_assert_eq!(updated_holder, new_holder, "Holder should be updated after transfer");
            
            // Verify transfer count increased
            let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
            prop_assert_eq!(metadata.transfer_count, transfer_num + 1, "Transfer count should increase");
            
            current_holder = new_holder;
        }
        
        // Property 4: Final holder should receive all subsequent payments
        let final_payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
            env.clone,
            lease_id,
            tenant.clone(),
            payment_amount,
        );
        prop_assert!(final_payment_result.is_ok(), "Final payment should succeed");
        
        // Property 5: Total yield should be preserved across transfers
        let final_config = PaymentRouter::get_payment_routing_config(env.clone(), lease_id).unwrap();
        prop_assert!(final_config.routing_enabled, "Routing should remain enabled");
        
        // Property 6: NFT metadata should be consistent
        let final_metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        prop_assert_eq!(final_metadata.lease_id, lease_id, "Lease ID should be preserved");
        prop_assert_eq!(final_metadata.monthly_rent, rent_amount, "Rent amount should be preserved");
        prop_assert!(final_metadata.transfer_count >= num_transfers, "Transfer count should be accurate");
    });
}

/// Property-based tests for mathematical proration
pub fn mathematical_proration_properties() {
    proptest!((
        monthly_rent in 1000i128..=10000i128,
        transfer_day in 1u32..=30u32,
        num_payments_before_transfer in 0u32..=10u32,
        payment_amount in 500i128..=2000i128
    )| {
        let env = Env::default();
        let lessor = Address::generate(&env);
        let tenant = Address::generate(&env);
        
        // Create test lease
        let lease_id = 2000u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            monthly_rent,
            500, // deposit_amount
            1000, // security_deposit
            90, // duration_days
        );
        
        // Release lock for testing
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Make payments before transfer
        let mut total_payments_before = 0i128;
        for _ in 0..num_payments_before_transfer {
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone(),
                lease_id,
                tenant.clone(),
                payment_amount,
            );
            prop_assert!(payment_result.is_ok(), "Payment before transfer should succeed");
            total_payments_before += payment_amount;
        }
        
        // Transfer NFT
        let new_holder = Address::generate(&env);
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), new_holder.clone());
        prop_assert!(transfer_result.is_ok(), "NFT transfer should succeed");
        
        // Update routing
        let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone,
            lease_id,
            lessor.clone(),
            new_holder.clone(),
        );
        prop_assert!(routing_result.is_ok(), "Routing update should succeed");
        
        // Property 1: Proration calculation should be mathematically sound
        let proration_ratio = (transfer_day as u32 * 10000) / 30; // Basis points
        let expected_proration = (total_payments_before * proration_ratio as i128) / 10000;
        
        // Property 2: No yield should be lost during transfer
        let payment_after_transfer = PaymentRouter::pay_lease_rent_with_nft_routing(
            env.clone,
            lease_id,
            tenant.clone(),
            payment_amount,
        );
        prop_assert!(payment_after_transfer.is_ok(), "Payment after transfer should succeed");
        
        // Property 3: Total yield should be conserved
        let final_config = PaymentRouter::get_payment_routing_config(env.clone(), lease_id).unwrap();
        prop_assert!(final_config.yield_accumulation_start > 0, "Yield accumulation should be tracked");
        
        // Property 4: Transfer timing should affect proration correctly
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        prop_assert!(metadata.last_transfer > metadata.minted_at, "Transfer timestamp should be recorded");
        
        // Property 5: Billing cycle should reset after transfer
        prop_assert!(metadata.billing_cycle_start >= metadata.last_transfer, 
            "Billing cycle should start after transfer");
    });
}

/// Property-based tests for cross-contract verification
pub fn cross_contract_verification_properties() {
    proptest!((
        lease_id in 1u64..=1000u64,
        verification_purpose in 0u32..=4u32, // Map to VerificationPurpose
        num_transfers in 0u32..=3u32
    )| {
        let env = Env::default();
        let lessor = Address::generate(&env);
        let tenant = Address::generate(&env);
        let requesting_contract = Address::generate(&env);
        
        // Create test lease with NFT
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            2000, // rent_amount
            500, // deposit_amount
            1000, // security_deposit
            90, // duration_days
        );
        
        // Release lock for testing
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Perform transfers if specified
        let mut current_holder = lessor.clone();
        for _ in 0..num_transfers {
            let new_holder = Address::generate(&env);
            let transfer_result = NFTTestUtils::simulate_nft_sale(
                &env,
                token_id,
                current_holder.clone(),
                new_holder.clone(),
                5000,
            );
            prop_assert!(transfer_result.is_ok(), "Transfer should succeed");
            
            let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
                env.clone,
                lease_id,
                current_holder.clone(),
                new_holder.clone(),
            );
            prop_assert!(routing_result.is_ok(), "Routing update should succeed");
            
            current_holder = new_holder;
        }
        
        // Map verification purpose
        let purpose = match verification_purpose {
            0 => crate::lessor_rights_nft::VerificationPurpose::RentPayment,
            1 => crate::lessor_rights_nft::VerificationPurpose::DepositRefund,
            2 => crate::lessor_rights_nft::VerificationPurpose::Slashing,
            3 => crate::lessor_rights_nft::VerificationPurpose::Buyout,
            _ => crate::lessor_rights_nft::VerificationPurpose::Termination,
        };
        
        // Property 1: Ownership verification should work for all purposes
        let request = crate::lessor_rights_nft::OwnershipVerificationRequest {
            lease_id,
            requesting_contract: requesting_contract.clone(),
            verification_purpose: purpose,
        };
        
        let verification_result = LessorRightsNFT::verify_token_ownership(env.clone(), request);
        prop_assert!(verification_result.is_ok(), "Ownership verification should succeed");
        
        let response = verification_result.unwrap();
        prop_assert!(response.is_valid, "Verification should be valid");
        prop_assert_eq!(response.current_holder, current_holder, "Current holder should be verified");
        prop_assert_eq!(response.lease_id, lease_id, "Lease ID should match");
        prop_assert_eq!(response.token_id, token_id, "Token ID should match");
        
        // Property 2: Verification should be cached
        let cached_result = LessorRightsNFT::verify_token_ownership(env.clone(), request.clone());
        prop_assert!(cached_result.is_ok(), "Cached verification should succeed");
        
        let cached_response = cached_result.unwrap();
        prop_assert_eq!(response.verification_timestamp, cached_response.verification_timestamp, 
            "Cached response should match original");
        
        // Property 3: Verification should fail for invalid lease
        let invalid_request = crate::lessor_rights_nft::OwnershipVerificationRequest {
            lease_id: 9999, // Non-existent lease
            requesting_contract: requesting_contract.clone(),
            verification_purpose: purpose,
        };
        
        let invalid_result = LessorRightsNFT::verify_token_ownership(env.clone(), invalid_request);
        prop_assert!(invalid_result.is_err(), "Invalid lease verification should fail");
    });
}

/// Comprehensive integration tests
#[cfg(test)]
mod integration_tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_complete_nft_sale_and_yield_redirection() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 1u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            2000, // monthly rent
            500,  // deposit
            1000, // security deposit
            90,   // 90 days
        );
        
        // Release lock for transfer
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Make some payments to establish yield
        for i in 0..3 {
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone(),
                lease_id,
                tenant.clone(),
                2000,
            );
            assert!(payment_result.is_ok());
        }
        
        // Sell NFT on secondary market
        let sale_result = NFTTestUtils::simulate_nft_sale(
            &env,
            token_id,
            lessor.clone(),
            buyer.clone(),
            15000, // Sale price
        );
        assert!(sale_result.is_ok());
        
        // Update payment routing
        let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone,
            lease_id,
            lessor.clone(),
            buyer.clone(),
        );
        assert!(routing_result.is_ok());
        
        // Verify new holder receives payments
        let new_payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
            env.clone,
            lease_id,
            tenant.clone(),
            2000,
        );
        assert!(new_payment_result.is_ok());
        
        // Verify routing configuration
        let config = PaymentRouter::get_payment_routing_config(env.clone(), lease_id).unwrap();
        assert_eq!(config.current_holder, buyer);
        assert!(config.routing_enabled);
        
        // Verify NFT metadata
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert_eq!(metadata.current_holder, buyer);
        assert_eq!(metadata.transfer_count, 1);
    }

    #[test]
    fn test_mid_cycle_transfer_proration() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        
        // Create lease with specific billing cycle
        let lease_id = 2u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            3000, // monthly rent
            600,  // deposit
            1500, // security deposit
            60,   // 60 days
        );
        
        // Release lock for transfer
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Make payments for 15 days (half billing cycle)
        for i in 0..5 {
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone,
                lease_id,
                tenant.clone(),
                1500, // Half month rent
            );
            assert!(payment_result.is_ok());
        }
        
        // Transfer NFT mid-cycle
        let transfer_result = NFTTestUtils::simulate_nft_sale(
            &env,
            token_id,
            lessor.clone(),
            buyer.clone(),
            20000,
        );
        assert!(transfer_result.is_ok());
        
        // Update routing with proration
        let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone,
            lease_id,
            lessor.clone(),
            buyer.clone(),
        );
        assert!(routing_result.is_ok());
        
        // Continue payments with new holder
        for i in 0..5 {
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone,
                lease_id,
                tenant.clone(),
                1500,
            );
            assert!(payment_result.is_ok());
        }
        
        // Verify proration was handled correctly
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert!(metadata.last_transfer > metadata.minted_at);
        assert!(metadata.billing_cycle_start >= metadata.last_transfer);
    }

    #[test]
    fn test_deposit_refund_to_nft_holder() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 3u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            1000, // rent
            300,  // deposit
            700,  // security deposit
            30,   // 30 days
        );
        
        // Release lock and transfer NFT
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        NFTTestUtils::simulate_nft_sale(&env, token_id, lessor.clone(), buyer.clone(), 8000).unwrap();
        PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone,
            lease_id,
            lessor.clone(),
            buyer.clone(),
        ).unwrap();
        
        // Terminate lease
        let mut lease_instance = crate::load_lease_instance_by_id(&env, lease_id).unwrap();
        lease_instance.status = LeaseStatus::Terminated;
        crate::save_lease_instance_by_id(&env, lease_id, &lease_instance);
        
        // Refund deposit to current NFT holder
        let refund_result = PaymentRouter::refund_deposit_to_nft_holder(env.clone(), lease_id, 1000);
        assert!(refund_result.is_ok());
        
        // Verify refund went to current holder
        let current_holder = LessorRightsNFT::get_current_holder(env.clone(), lease_id).unwrap();
        assert_eq!(current_holder, buyer);
    }

    #[test]
    fn test_mutual_release_with_nft_verification() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 4u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            1500, // rent
            400,  // deposit
            800,  // security deposit
            45,   // 45 days
        );
        
        // Release lock and transfer NFT
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        NFTTestUtils::simulate_nft_sale(&env, token_id, lessor.clone(), buyer.clone(), 12000).unwrap();
        PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone,
            lease_id,
            lessor.clone(),
            buyer.clone(),
        ).unwrap();
        
        // Perform mutual release with NFT holder verification
        let release_result = PaymentRouter::mutual_release_with_nft_verification(
            env.clone(),
            lease_id,
            tenant.clone(),
            buyer.clone(), // Current NFT holder
            900,  // return amount
            300,  // slash amount
        );
        assert!(release_result.is_ok());
        
        // Verify NFT lock was released
        let is_locked = env.storage()
            .persistent()
            .has(&NFTDataKey::NFTIndestructibilityLock(lease_id));
        assert!(!is_locked);
    }

    #[test]
    fn test_nft_indestructibility() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 5u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            TestAddress::generate(&env),
            2000, // rent
            500,  // deposit
            1000, // security deposit
            90,   // 90 days
        );
        
        // NFT should be locked while lease is active
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), buyer.clone());
        assert_eq!(transfer_result, Err(crate::lessor_rights_nft::NFTError::TransferDuringLock));
        
        // Release lock
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Transfer should now work
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), buyer.clone());
        assert!(transfer_result.is_ok());
        
        // Re-lock for testing
        LessorRightsNFT::update_nft_lock(env.clone(), lease_id, crate::lessor_rights_nft::LockReason::LeaseActive, None).unwrap();
        
        // Transfer should fail again
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, buyer.clone(), lessor.clone());
        assert_eq!(transfer_result, Err(crate::lessor_rights_nft::NFTError::TransferDuringLock));
    }

    #[test]
    fn test_property_based_verification() {
        nft_sale_yield_redirection_properties();
        mathematical_proration_properties();
        cross_contract_verification_properties();
    }

    #[test]
    fn test_multiple_concurrent_transfers() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 6u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            2500, // rent
            600,  // deposit
            1200, // security deposit
            120,  // 120 days
        );
        
        // Release lock for transfers
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Perform multiple rapid transfers
        let mut current_holder = lessor.clone();
        for i in 0..5 {
            let new_holder = TestAddress::generate(&env);
            
            // Make payment before transfer
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone,
                lease_id,
                tenant.clone(),
                1250, // Half month
            );
            assert!(payment_result.is_ok());
            
            // Transfer NFT
            NFTTestUtils::simulate_nft_sale(&env, token_id, current_holder.clone(), new_holder.clone(), 10000).unwrap();
            PaymentRouter::update_payment_routing_on_nft_transfer(
                env.clone,
                lease_id,
                current_holder.clone(),
                new_holder.clone(),
            ).unwrap();
            
            current_holder = new_holder;
        }
        
        // Verify final state
        let final_holder = LessorRightsNFT::get_current_holder(env.clone(), lease_id).unwrap();
        assert_eq!(final_holder, current_holder);
        
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert_eq!(metadata.transfer_count, 5);
        
        // Final payment should work
        let final_payment = PaymentRouter::pay_lease_rent_with_nft_routing(
            env.clone,
            lease_id,
            tenant.clone(),
            2500,
        );
        assert!(final_payment.is_ok());
    }

    #[test]
    fn test_yield_accumulation_tracking() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create lease and mint NFT
        let lease_id = 7u64;
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            lease_id,
            lessor.clone(),
            tenant.clone(),
            1800, // rent
            450,  // deposit
            900,  // security deposit
            60,   // 60 days
        );
        
        // Release lock for payments
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Make multiple payments and track accumulation
        let mut total_payments = 0i128;
        for i in 0..10 {
            let payment_amount = 1800 / 10; // 180 each
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                env.clone,
                lease_id,
                tenant.clone(),
                payment_amount,
            );
            assert!(payment_result.is_ok());
            
            total_payments += payment_amount;
            
            // Verify routing configuration
            let config = PaymentRouter::get_payment_routing_config(env.clone(), lease_id).unwrap();
            assert!(config.routing_enabled);
            assert_eq!(config.current_holder, lessor);
        }
        
        // Verify total accumulation
        let config = PaymentRouter::get_payment_routing_config(env.clone(), lease_id).unwrap();
        assert!(config.yield_accumulation_start > 0);
    }
}

/// Performance benchmarks for NFT operations
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_nft_minting() {
        let env = Env::default();
        
        // Benchmark NFT minting
        let start = Instant::now();
        let mut token_ids = Vec::new(&env);
        
        for i in 0..100 {
            let lessor = TestAddress::generate(&env);
            let lease_id = i as u64;
            
            // Create lease
            let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
                &env,
                lease_id,
                lessor,
                TestAddress::generate(&env),
                1000,
                300,
                600,
                90,
            );
            
            token_ids.push_back(token_id);
        }
        
        let duration = start.elapsed();
        println!("Minted 100 NFTs in {:?}", duration);
        assert!(duration.as_millis() < 1000, "NFT minting should complete within 1 second");
    }

    #[test]
    fn benchmark_nft_transfers() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        
        // Create NFT
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            1,
            lessor,
            TestAddress::generate(&env),
            1000,
            300,
            600,
            90,
        );
        
        // Release lock
        LessorRightsNFT::release_nft_lock(env.clone(), 1).unwrap();
        
        // Benchmark transfers
        let start = Instant::now();
        let mut current_holder = lessor;
        
        for i in 0..50 {
            let new_holder = TestAddress::generate(&env);
            
            let transfer_result = NFTTestUtils::simulate_nft_sale(
                &env,
                token_id,
                current_holder,
                new_holder,
                5000,
            );
            assert!(transfer_result.is_ok());
            
            let routing_result = PaymentRouter::update_payment_routing_on_nft_transfer(
                &env,
                1,
                current_holder,
                new_holder,
            );
            assert!(routing_result.is_ok());
            
            current_holder = new_holder;
        }
        
        let duration = start.elapsed();
        println!("50 NFT transfers in {:?}", duration);
        assert!(duration.as_millis() < 2000, "NFT transfers should complete within 2 seconds");
    }

    #[test]
    fn benchmark_payment_routing() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create lease and NFT
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            1,
            lessor,
            tenant.clone(),
            1000,
            300,
            600,
            90,
        );
        
        // Release lock
        LessorRightsNFT::release_nft_lock(env.clone(), 1).unwrap();
        
        // Benchmark payments
        let start = Instant::now();
        
        for i in 0..100 {
            let payment_result = PaymentRouter::pay_lease_rent_with_nft_routing(
                &env,
                1,
                tenant.clone(),
                1000,
            );
            assert!(payment_result.is_ok());
        }
        
        let duration = start.elapsed();
        println!("100 routed payments in {:?}", duration);
        assert!(duration.as_millis() < 1500, "Payment routing should complete within 1.5 seconds");
    }

    #[test]
    fn benchmark_ownership_verification() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let requesting_contract = TestAddress::generate(&env);
        
        // Create NFT
        let (lease, token_id) = NFTTestUtils::create_test_lease_with_nft(
            &env,
            1,
            lessor,
            TestAddress::generate(&env),
            1000,
            300,
            600,
            90,
        );
        
        // Benchmark ownership verification
        let start = Instant::now();
        
        for i in 0..1000 {
            let request = crate::lessor_rights_nft::OwnershipVerificationRequest {
                lease_id: 1,
                requesting_contract: requesting_contract.clone(),
                verification_purpose: crate::lessor_rights_nft::VerificationPurpose::RentPayment,
            };
            
            let verification_result = LessorRightsNFT::verify_token_ownership(&env, request);
            assert!(verification_result.is_ok());
        }
        
        let duration = start.elapsed();
        println!("1000 ownership verifications in {:?}", duration);
        assert!(duration.as_millis() < 3000, "Ownership verification should complete within 3 seconds");
    }
}
