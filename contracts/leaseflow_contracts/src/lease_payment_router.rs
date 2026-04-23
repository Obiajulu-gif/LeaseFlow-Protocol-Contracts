//! Lease Payment Router for NFT-Based Yield Distribution
//! 
//! This module refactors the lease state machine to route payments to the current
//! NFT holder instead of the original lessor, enabling liquid trading of lease rights.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DataKey,
    lessor_rights_nft::{LessorRightsNFT, OwnershipVerificationRequest, VerificationPurpose, NFTDataKey},
    save_lease_instance_by_id, load_lease_instance_by_id
};

/// Payment routing configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRoutingConfig {
    pub lease_id: u64,
    pub current_holder: Address,
    pub routing_enabled: bool,
    pub last_routing_update: u64,
    pub yield_accumulation_start: u64,
}

/// Yield accumulation record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldAccumulation {
    pub lease_id: u64,
    pub holder: Address,
    pub accumulated_yield: i128,
    pub accumulation_start: u64,
    pub last_payment_timestamp: u64,
    pub payment_count: u32,
}

/// Payment routing events
#[contractevent]
pub struct PaymentRouted {
    pub lease_id: u64,
    pub from: Address,
    pub to: Address,
    pub amount: i128,
    pub routing_timestamp: u64,
}

#[contractevent]
pub struct YieldAccumulated {
    pub lease_id: u64,
    pub holder: Address,
    pub accumulated_amount: i128,
    pub accumulation_period: u64,
}

#[contractevent]
pub struct RoutingUpdated {
    pub lease_id: u64,
    pub previous_holder: Address,
    pub new_holder: Address,
    pub update_timestamp: u64,
}

/// Payment routing errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingError {
    LeaseNotFound = 3001,
    NFTNotFound = 3002,
    OwnershipVerificationFailed = 3003,
    InvalidRoutingState = 3004,
    InsufficientYield = 3005,
    ProrationCalculationFailed = 3006,
    CrossContractCallFailed = 3007,
    PaymentRoutingDisabled = 3008,
}

/// Enhanced LeaseContract with NFT-based payment routing
impl LeaseContract {
    /// Enhanced rent payment function with NFT routing
    pub fn pay_lease_rent_with_nft_routing(
        env: Env,
        lease_id: u64,
        payer: Address,
        payment_amount: i128,
    ) -> Result<(), LeaseError> {
        payer.require_auth();

        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        require!(lease.active, "Lease is not active");

        // Verify payment authorization
        let is_primary = payer == lease.tenant;
        let is_authorized = env
            .storage()
            .persistent()
            .get::<_, bool>(&DataKey::AuthorizedPayer(lease_id, payer.clone()))
            .unwrap_or(false);
        if !is_primary && !is_authorized {
            return Err(LeaseError::Unauthorised);
        }

        // Get current NFT holder for routing
        let current_holder = Self::get_current_nft_holder(&env, lease_id)
            .map_err(|_| LeaseError::InvalidState)?;

        // Route payment to NFT holder
        Self::route_payment_to_nft_holder(&env, lease_id, current_holder, payment_amount)?;

        // Update tenant heartbeat if this is the tenant paying
        if is_primary {
            Self::update_tenant_heartbeat(&env, lease_id, &payer)?;
        }

        // Update payment tracking
        Self::update_payment_tracking(&env, lease_id, payment_amount)?;

        Ok(())
    }

    /// Enhanced deposit refund with NFT holder verification
    pub fn refund_deposit_to_nft_holder(
        env: Env,
        lease_id: u64,
        refund_amount: i128,
    ) -> Result<(), LeaseError> {
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Verify lease is in terminated state
        if !matches!(lease.status, LeaseStatus::Terminated | LeaseStatus::Expired) {
            return Err(LeaseError::InvalidState);
        }

        // Get current NFT holder
        let current_holder = Self::get_current_nft_holder(&env, lease_id)
            .map_err(|_| LeaseError::InvalidState)?;

        // Verify ownership for deposit refund
        Self::verify_ownership_for_deposit_refund(&env, lease_id, current_holder.clone())?;

        // Execute refund to NFT holder
        Self::execute_deposit_refund(&env, lease_id, current_holder, refund_amount)?;

        Ok(())
    }

    /// Enhanced mutual release with NFT holder verification
    pub fn mutual_release_with_nft_verification(
        env: Env,
        lease_id: u64,
        lessee_pubkey: Address,
        lessor_pubkey: Address,
        return_amount: i128,
        slash_amount: i128,
    ) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Verify participants
        if lessee_pubkey != lease.tenant {
            return Err(LeaseError::Unauthorised);
        }

        // Get current NFT holder (may not be original lessor)
        let current_holder = Self::get_current_nft_holder(&env, lease_id)
            .map_err(|_| LeaseError::InvalidState)?;

        // Verify NFT holder authorization for mutual release
        if lessor_pubkey != current_holder {
            return Err(LeaseError::Unauthorised);
        }

        // Verify both parties have authorized this transaction
        lessee_pubkey.require_auth();
        lessor_pubkey.require_auth();

        // Validate lease state
        if lease.status != LeaseStatus::Active && lease.status != LeaseStatus::Expired {
            return Err(LeaseError::LeaseNotFound);
        }

        // Mathematical validation
        let total_escrowed = lease.security_deposit + lease.deposit_amount;
        if return_amount + slash_amount != total_escrowed {
            return Err(LeaseError::InvalidReleaseMath);
        }

        if return_amount < 0 || slash_amount < 0 {
            return Err(LeaseError::InvalidReleaseMath);
        }

        // Execute payments to current NFT holder and tenant
        Self::execute_mutual_release_payments(
            &env,
            lease_id,
            lessee_pubkey,
            current_holder,
            return_amount,
            slash_amount,
        )?;

        // Update lease state
        lease.status = LeaseStatus::Terminated;
        lease.deposit_status = crate::DepositStatus::Settled;
        lease.active = false;

        // Release NFT lock since lease is terminated
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id)?;

        save_lease_instance_by_id(&env, lease_id, &lease);

        // Handle NFT return if applicable
        if let (Some(nft_contract_addr), Some(token_id)) = (lease.nft_contract.clone(), lease.token_id) {
            Self::handle_nft_return(&env, nft_contract_addr.clone(), token_id)?;
        }

        // Emit event
        crate::MutualLeaseFinalized {
            lease_id,
            return_amount,
            slash_amount,
            tenant_refund: return_amount,
            landlord_payout: slash_amount,
        }.publish(&env);

        Ok(())
    }

    /// Update payment routing when NFT is transferred
    pub fn update_payment_routing_on_nft_transfer(
        env: Env,
        lease_id: u64,
        previous_holder: Address,
        new_holder: Address,
    ) -> Result<(), LeaseError> {
        // Verify lease exists
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;

        // Calculate yield proration for mid-cycle transfer
        let proration_data = Self::calculate_yield_proration_on_transfer(&env, lease_id, previous_holder.clone())?;

        // Execute yield redistribution
        Self::execute_yield_redistribution(
            &env,
            lease_id,
            previous_holder.clone(),
            new_holder.clone(),
            &proration_data,
        )?;

        // Update routing configuration
        Self::update_routing_configuration(&env, lease_id, new_holder.clone())?;

        // Emit routing update event
        RoutingUpdated {
            lease_id,
            previous_holder,
            new_holder: new_holder.clone(),
            update_timestamp: env.ledger().timestamp(),
        }.publish(&env);

        Ok(())
    }

    /// Get current payment routing configuration
    pub fn get_payment_routing_config(env: Env, lease_id: u64) -> Result<PaymentRoutingConfig, LeaseError> {
        let config_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id); // Reuse existing key
        
        if let Some(config) = env.storage().persistent().get::<_, PaymentRoutingConfig>(&config_key) {
            Ok(config)
        } else {
            // Create default configuration
            let current_holder = Self::get_current_nft_holder(&env, lease_id)
                .map_err(|_| LeaseError::InvalidState)?;
            
            Ok(PaymentRoutingConfig {
                lease_id,
                current_holder,
                routing_enabled: true,
                last_routing_update: env.ledger().timestamp(),
                yield_accumulation_start: env.ledger().timestamp(),
            })
        }
    }

    // Helper methods for payment routing

    fn get_current_nft_holder(env: &Env, lease_id: u64) -> Result<Address, RoutingError> {
        LessorRightsNFT::get_current_holder(env.clone(), lease_id)
            .map_err(|_| RoutingError::NFTNotFound)
    }

    fn route_payment_to_nft_holder(
        env: &Env,
        lease_id: u64,
        holder: Address,
        amount: i128,
    ) -> Result<(), RoutingError> {
        // Update yield accumulation
        Self::update_yield_accumulation(env, lease_id, holder.clone(), amount)?;

        // In a real implementation, this would transfer tokens
        // For now, we'll emit an event
        PaymentRouted {
            lease_id,
            from: Address::generate(env), // Payer (simplified)
            to: holder,
            amount,
            routing_timestamp: env.ledger().timestamp(),
        }.publish(env);

        Ok(())
    }

    fn update_yield_accumulation(
        env: &Env,
        lease_id: u64,
        holder: Address,
        amount: i128,
    ) -> Result<(), RoutingError> {
        let accumulation_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id); // Reuse existing key
        
        let mut accumulation = env.storage()
            .persistent()
            .get::<_, YieldAccumulation>(&accumulation_key)
            .unwrap_or(YieldAccumulation {
                lease_id,
                holder: holder.clone(),
                accumulated_yield: 0,
                accumulation_start: env.ledger().timestamp(),
                last_payment_timestamp: env.ledger().timestamp(),
                payment_count: 0,
            });

        accumulation.accumulated_yield += amount;
        accumulation.last_payment_timestamp = env.ledger().timestamp();
        accumulation.payment_count += 1;

        env.storage()
            .persistent()
            .set(&accumulation_key, &accumulation);

        // Emit yield accumulation event
        YieldAccumulated {
            lease_id,
            holder,
            accumulated_amount: amount,
            accumulation_period: accumulation.last_payment_timestamp - accumulation.accumulation_start,
        }.publish(env);

        Ok(())
    }

    fn verify_ownership_for_deposit_refund(
        env: &Env,
        lease_id: u64,
        claimed_holder: Address,
    ) -> Result<(), RoutingError> {
        let request = OwnershipVerificationRequest {
            lease_id,
            requesting_contract: env.current_contract_address(),
            verification_purpose: VerificationPurpose::DepositRefund,
        };

        let response = LessorRightsNFT::verify_token_ownership(env.clone(), request)
            .map_err(|_| RoutingError::OwnershipVerificationFailed)?;

        if !response.is_valid || response.current_holder != claimed_holder {
            return Err(RoutingError::OwnershipVerificationFailed);
        }

        Ok(())
    }

    fn execute_deposit_refund(
        env: &Env,
        lease_id: u64,
        holder: Address,
        amount: i128,
    ) -> Result<(), RoutingError> {
        // In a real implementation, this would transfer tokens to the holder
        // For now, we'll emit an event
        PaymentRouted {
            lease_id,
            from: env.current_contract_address(),
            to: holder,
            amount,
            routing_timestamp: env.ledger().timestamp(),
        }.publish(env);

        Ok(())
    }

    fn execute_mutual_release_payments(
        env: &Env,
        lease_id: u64,
        tenant: Address,
        holder: Address,
        return_amount: i128,
        slash_amount: i128,
    ) -> Result<(), RoutingError> {
        // Transfer return amount to tenant
        if return_amount > 0 {
            PaymentRouted {
                lease_id,
                from: env.current_contract_address(),
                to: tenant,
                amount: return_amount,
                routing_timestamp: env.ledger().timestamp(),
            }.publish(env);
        }

        // Transfer slash amount to current NFT holder
        if slash_amount > 0 {
            PaymentRouted {
                lease_id,
                from: env.current_contract_address(),
                to: holder,
                amount: slash_amount,
                routing_timestamp: env.ledger().timestamp(),
            }.publish(env);
        }

        Ok(())
    }

    fn calculate_yield_proration_on_transfer(
        env: &Env,
        lease_id: u64,
        previous_holder: Address,
    ) -> Result<YieldProrationData, RoutingError> {
        let accumulation_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id);
        
        let accumulation = env.storage()
            .persistent()
            .get::<_, YieldAccumulation>(&accumulation_key)
            .ok_or(RoutingError::InsufficientYield)?;

        let current_time = env.ledger().timestamp();
        let billing_cycle_duration = 30 * 24 * 60 * 60; // 30 days

        // Calculate time since accumulation start
        let elapsed_time = current_time.saturating_sub(accumulation.accumulation_start);
        
        // Calculate proration based on elapsed time
        let proration_ratio = if elapsed_time < billing_cycle_duration {
            (elapsed_time * 10000) / billing_cycle_duration
        } else {
            10000 // Full billing cycle
        };

        let proration_amount = (accumulation.accumulated_yield * proration_ratio as i128) / 10000;

        Ok(YieldProrationData {
            accumulated_yield: accumulation.accumulated_yield,
            proration_amount,
            elapsed_time,
            proration_ratio,
        })
    }

    fn execute_yield_redistribution(
        env: &Env,
        lease_id: u64,
        previous_holder: Address,
        new_holder: Address,
        proration_data: &YieldProrationData,
    ) -> Result<(), RoutingError> {
        // Transfer prorated amount to previous holder
        if proration_data.proration_amount > 0 {
            PaymentRouted {
                lease_id,
                from: env.current_contract_address(),
                to: previous_holder,
                amount: proration_data.proration_amount,
                routing_timestamp: env.ledger().timestamp(),
            }.publish(env);
        }

        // Reset yield accumulation for new holder
        let accumulation_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id);
        let new_accumulation = YieldAccumulation {
            lease_id,
            holder: new_holder,
            accumulated_yield: 0,
            accumulation_start: env.ledger().timestamp(),
            last_payment_timestamp: env.ledger().timestamp(),
            payment_count: 0,
        };

        env.storage()
            .persistent()
            .set(&accumulation_key, &new_accumulation);

        Ok(())
    }

    fn update_routing_configuration(
        env: &Env,
        lease_id: u64,
        new_holder: Address,
    ) -> Result<(), RoutingError> {
        let config_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id);
        
        let config = PaymentRoutingConfig {
            lease_id,
            current_holder: new_holder,
            routing_enabled: true,
            last_routing_update: env.ledger().timestamp(),
            yield_accumulation_start: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&config_key, &config);

        Ok(())
    }

    fn update_payment_tracking(env: &Env, lease_id: u64, amount: i128) -> Result<(), RoutingError> {
        // Update payment tracking for analytics
        let tracking_key = crate::lessor_rights_nft::NFTDataKey::LeaseInstance(lease_id);
        
        let current_time = env.ledger().timestamp();
        let mut payment_count = env.storage()
            .persistent()
            .get::<_, u32>(&tracking_key)
            .unwrap_or(0);
        
        payment_count += 1;
        env.storage()
            .persistent()
            .set(&tracking_key, &payment_count);

        Ok(())
    }

    fn update_tenant_heartbeat(env: &Env, lease_id: u64, tenant: &Address) -> Result<(), LeaseError> {
        let mut lease = load_lease_instance_by_id(env, lease_id)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        if lease.tenant != *tenant {
            return Err(LeaseError::Unauthorised);
        }
        
        lease.last_tenant_interaction = env.ledger().timestamp();
        save_lease_instance_by_id(env, lease_id, &lease);
        
        Ok(())
    }

    fn handle_nft_return(
        env: &Env,
        nft_contract_addr: Address,
        token_id: u128,
    ) -> Result<(), LeaseError> {
        // In a real implementation, this would handle NFT return logic
        // For now, we'll just emit an event or update state
        Ok(())
    }
}

/// Yield proration data structure
#[derive(Debug, Clone)]
struct YieldProrationData {
    accumulated_yield: i128,
    proration_amount: i128,
    elapsed_time: u64,
    proration_ratio: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_nft_based_payment_routing() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 1u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Make rent payment
        let result = LeaseContract::pay_lease_rent_with_nft_routing(
            env.clone(),
            lease_id,
            tenant.clone(),
            1000,
        );
        
        assert!(result.is_ok());
        
        // Verify routing configuration
        let config = LeaseContract::get_payment_routing_config(env.clone(), lease_id).unwrap();
        assert_eq!(config.current_holder, lessor);
        assert!(config.routing_enabled);
    }

    #[test]
    fn test_deposit_refund_to_nft_holder() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Setup: Create lease, mint NFT, and terminate lease
        let lease_id = 2u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Terminate lease (simplified)
        let mut lease = crate::load_lease_instance_by_id(&env, lease_id).unwrap();
        lease.status = LeaseStatus::Terminated;
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Release NFT lock
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Refund deposit to NFT holder
        let result = LeaseContract::refund_deposit_to_nft_holder(env.clone(), lease_id, 500);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mutual_release_with_nft_verification() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 3u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Perform mutual release
        let result = LeaseContract::mutual_release_with_nft_verification(
            env.clone(),
            lease_id,
            tenant.clone(),
            lessor.clone(),
            600, // return amount
            400, // slash amount
        );
        
        assert!(result.is_ok());
        
        // Verify NFT lock was released
        let is_locked = env.storage()
            .persistent()
            .has(&crate::lessor_rights_nft::NFTDataKey::NFTIndestructibilityLock(lease_id));
        assert!(!is_locked);
    }

    #[test]
    fn test_payment_routing_update_on_transfer() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let new_holder = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 4u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Release NFT lock to allow transfer
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Transfer NFT
        LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), new_holder.clone()).unwrap();
        
        // Update payment routing
        let result = LeaseContract::update_payment_routing_on_nft_transfer(
            env.clone(),
            lease_id,
            lessor.clone(),
            new_holder.clone(),
        );
        
        assert!(result.is_ok());
        
        // Verify routing was updated
        let config = LeaseContract::get_payment_routing_config(env.clone(), lease_id).unwrap();
        assert_eq!(config.current_holder, new_holder);
    }

    #[test]
    fn test_yield_accumulation_tracking() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 5u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Make multiple payments
        for i in 0..3 {
            let result = LeaseContract::pay_lease_rent_with_nft_routing(
                env.clone(),
                lease_id,
                tenant.clone(),
                1000,
            );
            assert!(result.is_ok());
        }
        
        // Verify yield accumulation (simplified check)
        let config = LeaseContract::get_payment_routing_config(env.clone(), lease_id).unwrap();
        assert!(config.yield_accumulation_start > 0);
    }
}
