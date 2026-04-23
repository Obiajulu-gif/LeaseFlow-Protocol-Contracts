//! Lessor Rights Tokenization (Yield NFT)
//! 
//! This module transforms static lease agreements into liquid, tradable financial assets
//! by tokenizing lessor rights as Soroban-compliant NFTs with embedded lease metadata.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DataKey,
    save_lease_instance_by_id, load_lease_instance_by_id
};

/// Soroban-compliant NFT metadata structure for lessor rights
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LessorRightsNFTMetadata {
    /// Unique lease identifier
    pub lease_id: u64,
    /// Original lessor address
    pub original_lessor: Address,
    /// Current token holder
    pub current_holder: Address,
    /// Lease start timestamp
    pub lease_start: u64,
    /// Lease end timestamp
    pub lease_end: u64,
    /// Monthly rent amount
    pub monthly_rent: i128,
    /// Security deposit amount
    pub security_deposit: i128,
    /// Property URI hash for privacy
    pub property_hash: BytesN<32>,
    /// Token minted timestamp
    pub minted_at: u64,
    /// Last transfer timestamp
    pub last_transfer: u64,
    /// Transfer count
    pub transfer_count: u32,
    /// Yield accumulated since last transfer
    pub pending_yield: i128,
    /// Billing cycle start for proration
    pub billing_cycle_start: u64,
}

/// NFT transfer record for mid-cycle proration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTTransferRecord {
    pub token_id: u128,
    pub from_holder: Address,
    pub to_holder: Address,
    pub transfer_timestamp: u64,
    pub accrued_rent_at_transfer: i128,
    pub proration_amount: i128,
    pub billing_cycle_remaining: u64,
}

/// Cross-contract call interface for NFT ownership verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipVerificationRequest {
    pub lease_id: u64,
    pub requesting_contract: Address,
    pub verification_purpose: VerificationPurpose,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationPurpose {
    RentPayment,
    DepositRefund,
    Slashing,
    Buyout,
    Termination,
}

/// NFT ownership verification response
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipVerificationResponse {
    pub is_valid: bool,
    pub current_holder: Address,
    pub verification_timestamp: u64,
    pub lease_id: u64,
    pub token_id: u128,
}

/// NFT indestructibility lock
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTIndestructibilityLock {
    pub lease_id: u64,
    pub token_id: u128,
    pub lock_reason: LockReason,
    pub locked_at: u64,
    pub expires_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LockReason {
    LeaseActive,
    LeaseDisputed,
    ArbitrationInProgress,
    RegulatoryHold,
}

/// Events for NFT operations
#[contractevent]
pub struct LessorRightsTokenized {
    pub lease_id: u64,
    pub token_id: u128,
    pub original_lessor: Address,
    pub minted_at: u64,
}

#[contractevent]
pub struct NFTTransferred {
    pub token_id: u128,
    pub from_holder: Address,
    pub to_holder: Address,
    pub transfer_timestamp: u64,
    pub accrued_yield: i128,
    pub proration_amount: i128,
}

#[contractevent]
pub struct YieldRedirected {
    pub token_id: u128,
    pub new_holder: Address,
    pub yield_amount: i128,
    pub redirection_timestamp: u64,
}

#[contractevent]
pub struct NFTLocked {
    pub token_id: u128,
    pub lease_id: u64,
    pub lock_reason: LockReason,
    pub locked_at: u64,
}

/// NFT-related errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NFTError {
    LeaseNotFound = 2001,
    NFTAlreadyExists = 2002,
    InvalidLeaseState = 2003,
    UnauthorizedTransfer = 2004,
    NFTNotFound = 2005,
    TransferDuringLock = 2006,
    InvalidProration = 2007,
    OwnershipVerificationFailed = 2008,
    CrossContractCallFailed = 2009,
    InsufficientYield = 2010,
    TokenIndestructible = 2011,
    MetadataCorruption = 2012,
}

/// Extended DataKey for NFT operations
#[contracttype]
#[derive(Debug, Clone)]
pub enum NFTDataKey {
    // Original data keys
    Lease(Symbol),
    LeaseInstance(u64),
    Receipt(Symbol, u32),
    Admin,
    UsageRights(Address, u128),
    HistoricalLease(u64),
    KycProvider,
    AllowedAsset(Address),
    AuthorizedPayer(u64, Address),
    RoommateBalance(u64, Address),
    PlatformFeeAmount,
    PlatformFeeToken,
    PlatformFeeRecipient,
    TermsHash,
    WhitelistedOracle(BytesN<32>),
    OracleNonce(BytesN<32>, u64),
    TenantFlag(u64),
    
    // NFT-related keys
    LessorRightsNFT(u128),
    NFTMetadata(u128),
    NFTTransferRecord(u128, u64), // token_id, transfer_index
    NFTIndestructibilityLock(u64), // lease_id
    LeaseToNFTMapping(u64), // lease_id -> token_id
    HolderToNFTs(Address), // holder -> Vec<token_id>
    OwnershipVerificationCache(u64, Address), // lease_id, requester
}

/// Lessor Rights NFT implementation
pub struct LessorRightsNFT;

impl LessorRightsNFT {
    /// Mint lessor rights NFT during lease initialization
    pub fn mint_lessor_rights_token(
        env: Env,
        lease_id: u64,
        lessor: Address,
    ) -> Result<u128, NFTError> {
        // Verify lease exists and is in appropriate state
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(NFTError::LeaseNotFound)?;
        
        // Verify lease is in a state that allows tokenization
        if !Self::is_lease_tokenizable(&lease) {
            return Err(NFTError::InvalidLeaseState);
        }
        
        // Check if NFT already exists for this lease
        if env.storage().persistent().has(&NFTDataKey::LeaseToNFTMapping(lease_id)) {
            return Err(NFTError::NFTAlreadyExists);
        }
        
        // Generate unique token ID
        let token_id = Self::generate_unique_token_id(&env, lease_id);
        
        // Create NFT metadata
        let metadata = Self::create_nft_metadata(&env, &lease, lease_id, lessor.clone(), token_id)?;
        
        // Store NFT data
        Self::store_nft_data(&env, token_id, &metadata, lease_id)?;
        
        // Create indestructibility lock
        Self::create_indestructibility_lock(&env, lease_id, token_id, LockReason::LeaseActive)?;
        
        // Update lease instance to reference NFT
        Self::update_lease_for_nft(&env, lease_id, token_id)?;
        
        // Emit tokenization event
        LessorRightsTokenized {
            lease_id,
            token_id,
            original_lessor: lessor,
            minted_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(token_id)
    }
    
    /// Transfer NFT with yield proration
    pub fn transfer_nft(
        env: Env,
        token_id: u128,
        from_holder: Address,
        to_holder: Address,
    ) -> Result<(), NFTError> {
        // Verify NFT exists
        let metadata = env.storage()
            .persistent()
            .get::<_, LessorRightsNFTMetadata>(&NFTDataKey::NFTMetadata(token_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        // Verify current holder
        if metadata.current_holder != from_holder {
            return Err(NFTError::UnauthorizedTransfer);
        }
        
        // Check if NFT is locked
        if Self::is_nft_locked(&env, metadata.lease_id) {
            return Err(NFTError::TransferDuringLock);
        }
        
        // Calculate yield proration for mid-cycle transfer
        let proration_data = Self::calculate_yield_proration(&env, &metadata)?;
        
        // Create transfer record
        let transfer_record = NFTTransferRecord {
            token_id,
            from_holder: from_holder.clone(),
            to_holder: to_holder.clone(),
            transfer_timestamp: env.ledger().timestamp(),
            accrued_rent_at_transfer: proration_data.accrued_rent,
            proration_amount: proration_data.proration_amount,
            billing_cycle_remaining: proration_data.billing_cycle_remaining,
        };
        
        // Store transfer record
        Self::store_transfer_record(&env, token_id, &transfer_record)?;
        
        // Execute yield redirection
        Self::execute_yield_redirection(&env, token_id, from_holder.clone(), to_holder.clone(), &proration_data)?;
        
        // Update NFT metadata
        Self::update_nft_metadata_on_transfer(&env, token_id, to_holder.clone())?;
        
        // Update holder mappings
        Self::update_holder_mappings(&env, token_id, from_holder, to_holder)?;
        
        // Emit transfer event
        NFTTransferred {
            token_id,
            from_holder,
            to_holder: to_holder.clone(),
            transfer_timestamp: env.ledger().timestamp(),
            accrued_yield: proration_data.accrued_rent,
            proration_amount: proration_data.proration_amount,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Verify token ownership (cross-contract call)
    pub fn verify_token_ownership(
        env: Env,
        request: OwnershipVerificationRequest,
    ) -> Result<OwnershipVerificationResponse, NFTError> {
        // Get token ID from lease
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&NFTDataKey::LeaseToNFTMapping(request.lease_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        // Get NFT metadata
        let metadata = env.storage()
            .persistent()
            .get::<_, LessorRightsNFTMetadata>(&NFTDataKey::NFTMetadata(token_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        // Verify lease state for the purpose
        Self::verify_purpose_validity(&env, request.lease_id, &request.verification_purpose)?;
        
        // Create verification response
        let response = OwnershipVerificationResponse {
            is_valid: true,
            current_holder: metadata.current_holder,
            verification_timestamp: env.ledger().timestamp(),
            lease_id: request.lease_id,
            token_id,
        };
        
        // Cache verification result
        Self::cache_verification_result(&env, &request, &response)?;
        
        Ok(response)
    }
    
    /// Get current NFT holder for a lease
    pub fn get_current_holder(env: Env, lease_id: u64) -> Result<Address, NFTError> {
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&NFTDataKey::LeaseToNFTMapping(lease_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        let metadata = env.storage()
            .persistent()
            .get::<_, LessorRightsNFTMetadata>(&NFTDataKey::NFTMetadata(token_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        Ok(metadata.current_holder)
    }
    
    /// Get NFT metadata
    pub fn get_nft_metadata(env: Env, token_id: u128) -> Result<LessorRightsNFTMetadata, NFTError> {
        env.storage()
            .persistent()
            .get::<_, LessorRightsNFTMetadata>(&NFTDataKey::NFTMetadata(token_id))
            .ok_or(NFTError::NFTNotFound)
    }
    
    /// Get all NFTs owned by an address
    pub fn get_holder_nfts(env: Env, holder: Address) -> Vec<u128> {
        env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&NFTDataKey::HolderToNFTs(holder))
            .unwrap_or_else(|| Vec::new(&env))
    }
    
    /// Update NFT lock status
    pub fn update_nft_lock(
        env: Env,
        lease_id: u64,
        lock_reason: LockReason,
        expires_at: Option<u64>,
    ) -> Result<(), NFTError> {
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&NFTDataKey::LeaseToNFTMapping(lease_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        let lock = NFTIndestructibilityLock {
            lease_id,
            token_id,
            lock_reason,
            locked_at: env.ledger().timestamp(),
            expires_at,
        };
        
        env.storage()
            .persistent()
            .set(&NFTDataKey::NFTIndestructibilityLock(lease_id), &lock);
        
        // Emit lock event
        NFTLocked {
            token_id,
            lease_id,
            lock_reason,
            locked_at: lock.locked_at,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Release NFT lock (when lease is no longer active/disputed)
    pub fn release_nft_lock(env: Env, lease_id: u64) -> Result<(), NFTError> {
        if !env.storage().persistent().has(&NFTDataKey::NFTIndestructibilityLock(lease_id)) {
            return Ok(()); // Already released
        }
        
        env.storage()
            .persistent()
            .remove(&NFTDataKey::NFTIndestructibilityLock(lease_id));
        
        Ok(())
    }
    
    // Helper methods
    
    fn is_lease_tokenizable(lease: &LeaseInstance) -> bool {
        matches!(lease.status, LeaseStatus::Active | LeaseStatus::Pending)
    }
    
    fn generate_unique_token_id(env: &Env, lease_id: u64) -> u128 {
        // Generate unique token ID based on lease ID and timestamp
        let timestamp = env.ledger().timestamp();
        ((lease_id as u128) << 64) | (timestamp as u128)
    }
    
    fn create_nft_metadata(
        env: &Env,
        lease: &LeaseInstance,
        lease_id: u64,
        lessor: Address,
        token_id: u128,
    ) -> Result<LessorRightsNFTMetadata, NFTError> {
        let property_hash = Self::compute_property_hash(env, &lease.property_uri);
        
        Ok(LessorRightsNFTMetadata {
            lease_id,
            original_lessor: lessor,
            current_holder: lessor,
            lease_start: lease.start_date,
            lease_end: lease.end_date,
            monthly_rent: lease.rent_amount,
            security_deposit: lease.deposit_amount + lease.security_deposit,
            property_hash,
            minted_at: env.ledger().timestamp(),
            last_transfer: env.ledger().timestamp(),
            transfer_count: 0,
            pending_yield: 0,
            billing_cycle_start: lease.start_date,
        })
    }
    
    fn compute_property_hash(env: &Env, property_uri: &String) -> BytesN<32> {
        let data = property_uri.to_val();
        let hash = env.crypto().sha256(&data);
        BytesN::from_array(&hash)
    }
    
    fn store_nft_data(
        env: &Env,
        token_id: u128,
        metadata: &LessorRightsNFTMetadata,
        lease_id: u64,
    ) -> Result<(), NFTError> {
        // Store NFT metadata
        env.storage()
            .persistent()
            .set(&NFTDataKey::NFTMetadata(token_id), metadata);
        
        // Store lease to NFT mapping
        env.storage()
            .persistent()
            .set(&NFTDataKey::LeaseToNFTMapping(lease_id), &token_id);
        
        // Update holder mappings
        let mut holder_nfts = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&NFTDataKey::HolderToNFTs(metadata.current_holder.clone()))
            .unwrap_or_else(|| Vec::new(env));
        holder_nfts.push_back(token_id);
        env.storage()
            .persistent()
            .set(&NFTDataKey::HolderToNFTs(metadata.current_holder.clone()), &holder_nfts);
        
        // Set TTL for all entries
        let key = NFTDataKey::NFTMetadata(token_id);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60); // 1 year
        
        Ok(())
    }
    
    fn create_indestructibility_lock(
        env: &Env,
        lease_id: u64,
        token_id: u128,
        lock_reason: LockReason,
    ) -> Result<(), NFTError> {
        let lock = NFTIndestructibilityLock {
            lease_id,
            token_id,
            lock_reason,
            locked_at: env.ledger().timestamp(),
            expires_at: None, // Indefinite while lease is active
        };
        
        env.storage()
            .persistent()
            .set(&NFTDataKey::NFTIndestructibilityLock(lease_id), &lock);
        
        Ok(())
    }
    
    fn update_lease_for_nft(env: &Env, lease_id: u64, token_id: u128) -> Result<(), NFTError> {
        let mut lease = load_lease_instance_by_id(env, lease_id)
            .ok_or(NFTError::LeaseNotFound)?;
        
        // Update lease to reference NFT
        lease.nft_contract = Some(env.current_contract_address());
        lease.token_id = Some(token_id);
        
        save_lease_instance_by_id(env, lease_id, &lease);
        
        Ok(())
    }
    
    fn is_nft_locked(env: &Env, lease_id: u64) -> bool {
        env.storage()
            .persistent()
            .has(&NFTDataKey::NFTIndestructibilityLock(lease_id))
    }
    
    fn calculate_yield_proration(
        env: &Env,
        metadata: &LessorRightsNFTMetadata,
    ) -> Result<YieldProrationData, NFTError> {
        let current_time = env.ledger().timestamp();
        let billing_cycle_duration = 30 * 24 * 60 * 60; // 30 days
        
        // Calculate time since last transfer/billing cycle start
        let time_elapsed = current_time.saturating_sub(metadata.billing_cycle_start);
        
        // Calculate accrued rent
        let accrued_rent = (time_elapsed * metadata.monthly_rent) / billing_cycle_duration;
        
        // Calculate remaining time in billing cycle
        let billing_cycle_remaining = billing_cycle_duration.saturating_sub(time_elapsed);
        
        // Calculate proration amount (accrued rent to be paid to previous holder)
        let proration_amount = accrued_rent;
        
        Ok(YieldProrationData {
            accrued_rent,
            proration_amount,
            billing_cycle_remaining,
            time_elapsed,
        })
    }
    
    fn store_transfer_record(env: &Env, token_id: u128, record: &NFTTransferRecord) -> Result<(), NFTError> {
        let transfer_index = record.transfer_count;
        env.storage()
            .persistent()
            .set(&NFTDataKey::NFTTransferRecord(token_id, transfer_index), record);
        
        Ok(())
    }
    
    fn execute_yield_redirection(
        env: &Env,
        token_id: u128,
        from_holder: Address,
        to_holder: Address,
        proration_data: &YieldProrationData,
    ) -> Result<(), NFTError> {
        // In a real implementation, this would transfer the accrued yield
        // For now, we'll emit an event and update accounting
        
        // Emit yield redirection event
        YieldRedirected {
            token_id,
            new_holder: to_holder.clone(),
            yield_amount: proration_data.proration_amount,
            redirection_timestamp: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    fn update_nft_metadata_on_transfer(
        env: &Env,
        token_id: u128,
        new_holder: Address,
    ) -> Result<(), NFTError> {
        let mut metadata = env.storage()
            .persistent()
            .get::<_, LessorRightsNFTMetadata>(&NFTDataKey::NFTMetadata(token_id))
            .ok_or(NFTError::NFTNotFound)?;
        
        // Update metadata
        metadata.current_holder = new_holder;
        metadata.last_transfer = env.ledger().timestamp();
        metadata.transfer_count += 1;
        metadata.pending_yield = 0;
        metadata.billing_cycle_start = env.ledger().timestamp();
        
        // Store updated metadata
        env.storage()
            .persistent()
            .set(&NFTDataKey::NFTMetadata(token_id), &metadata);
        
        Ok(())
    }
    
    fn update_holder_mappings(
        env: &Env,
        token_id: u128,
        from_holder: Address,
        to_holder: Address,
    ) -> Result<(), NFTError> {
        // Remove from previous holder
        let mut from_holder_nfts = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&NFTDataKey::HolderToNFTs(from_holder.clone()))
            .unwrap_or_else(|| Vec::new(env));
        
        // Remove token ID from previous holder's list
        from_holder_nfts.retain(|&id| id != token_id);
        env.storage()
            .persistent()
            .set(&NFTDataKey::HolderToNFTs(from_holder), &from_holder_nfts);
        
        // Add to new holder
        let mut to_holder_nfts = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&NFTDataKey::HolderToNFTs(to_holder.clone()))
            .unwrap_or_else(|| Vec::new(env));
        to_holder_nfts.push_back(token_id);
        env.storage()
            .persistent()
            .set(&NFTDataKey::HolderToNFTs(to_holder), &to_holder_nfts);
        
        Ok(())
    }
    
    fn verify_purpose_validity(
        env: &Env,
        lease_id: u64,
        purpose: &VerificationPurpose,
    ) -> Result<(), NFTError> {
        let lease = load_lease_instance_by_id(env, lease_id)
            .ok_or(NFTError::LeaseNotFound)?;
        
        match purpose {
            VerificationPurpose::RentPayment => {
                if !matches!(lease.status, LeaseStatus::Active) {
                    return Err(NFTError::InvalidLeaseState);
                }
            }
            VerificationPurpose::DepositRefund | VerificationPurpose::Slashing | VerificationPurpose::Termination => {
                if !matches!(lease.status, LeaseStatus::Terminated | LeaseStatus::Expired) {
                    return Err(NFTError::InvalidLeaseState);
                }
            }
            VerificationPurpose::Buyout => {
                if !matches!(lease.status, LeaseStatus::Active) {
                    return Err(NFTError::InvalidLeaseState);
                }
            }
        }
        
        Ok(())
    }
    
    fn cache_verification_result(
        env: &Env,
        request: &OwnershipVerificationRequest,
        response: &OwnershipVerificationResponse,
    ) -> Result<(), NFTError> {
        // Cache for 5 minutes
        let cache_key = NFTDataKey::OwnershipVerificationCache(request.lease_id, request.requesting_contract.clone());
        env.storage()
            .temporary()
            .set(&cache_key, response);
        env.storage()
            .temporary()
            .extend_ttl(&cache_key, 300, 300); // 5 minutes
        
        Ok(())
    }
}

/// Yield proration calculation data
#[derive(Debug, Clone)]
struct YieldProrationData {
    accrued_rent: i128,
    proration_amount: i128,
    billing_cycle_remaining: u64,
    time_elapsed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_mint_lessor_rights_nft() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let tenant = TestAddress::generate(&env);
        
        // Create a test lease
        let lease_id = 1u64;
        let lease = crate::LeaseInstance {
            landlord: lessor,
            tenant,
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + (30 * 24 * 60 * 60),
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            // ... other fields with default values
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + (30 * 24 * 60 * 60),
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: crate::DepositStatus::Held,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() + (30 * 24 * 60 * 60),
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: Vec::new(&env),
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
            last_tenant_interaction: env.ledger().timestamp(),
        };
        
        // Store lease
        crate::save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Mint NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Verify NFT was created
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert_eq!(metadata.lease_id, lease_id);
        assert_eq!(metadata.original_lessor, lessor);
        assert_eq!(metadata.current_holder, lessor);
        assert_eq!(metadata.monthly_rent, 1000);
        
        // Verify lease was updated
        let updated_lease = crate::load_lease_instance_by_id(&env, lease_id).unwrap();
        assert_eq!(updated_lease.nft_contract, Some(env.current_contract_address()));
        assert_eq!(updated_lease.token_id, Some(token_id));
    }

    #[test]
    fn test_nft_transfer_with_proration() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let new_holder = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 2u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Transfer NFT
        LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), new_holder.clone()).unwrap();
        
        // Verify transfer
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert_eq!(metadata.current_holder, new_holder);
        assert_eq!(metadata.transfer_count, 1);
        
        // Verify holder mappings
        let lessor_nfts = LessorRightsNFT::get_holder_nfts(env.clone(), lessor.clone());
        let new_holder_nfts = LessorRightsNFT::get_holder_nfts(env.clone(), new_holder.clone());
        
        assert!(!lessor_nfts.contains(&token_id));
        assert!(new_holder_nfts.contains(&token_id));
    }

    #[test]
    fn test_token_ownership_verification() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let requesting_contract = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 3u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Verify ownership
        let request = OwnershipVerificationRequest {
            lease_id,
            requesting_contract: requesting_contract.clone(),
            verification_purpose: VerificationPurpose::RentPayment,
        };
        
        let response = LessorRightsNFT::verify_token_ownership(env.clone(), request).unwrap();
        
        assert!(response.is_valid);
        assert_eq!(response.current_holder, lessor);
        assert_eq!(response.lease_id, lease_id);
        assert_eq!(response.token_id, token_id);
    }

    #[test]
    fn test_nft_indestructibility_lock() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 4u64;
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // NFT should be locked by default
        let new_holder = TestAddress::generate(&env);
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), new_holder.clone());
        
        // Transfer should fail due to lock
        assert_eq!(transfer_result, Err(NFTError::TransferDuringLock));
        
        // Release lock
        LessorRightsNFT::release_nft_lock(env.clone(), lease_id).unwrap();
        
        // Transfer should now succeed
        let transfer_result = LessorRightsNFT::transfer_nft(env.clone(), token_id, lessor.clone(), new_holder.clone());
        assert!(transfer_result.is_ok());
    }

    #[test]
    fn test_get_current_holder() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        
        // Setup: Create lease and mint NFT
        let lease_id = 5u64;
        LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        
        // Get current holder
        let holder = LessorRightsNFT::get_current_holder(env.clone(), lease_id).unwrap();
        assert_eq!(holder, lessor);
    }

    #[test]
    fn test_get_holder_nfts() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        
        // Setup: Create multiple leases and mint NFTs
        for i in 0..3 {
            let lease_id = (i + 10) as u64;
            LessorRightsNFT::mint_lessor_rights_token(env.clone(), lease_id, lessor.clone()).unwrap();
        }
        
        // Get holder's NFTs
        let nfts = LessorRightsNFT::get_holder_nfts(env.clone(), lessor.clone());
        assert_eq!(nfts.len(), 3);
    }
}
