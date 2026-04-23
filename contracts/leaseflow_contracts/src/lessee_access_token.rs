//! Lessee Access Token Minting (Utility NFT)
//! 
//! This module provides a verifiable, cryptographic proof-of-access for lessees
//! to unlock and utilize rented digital or physical assets through Utility NFTs.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DepositStatus,
    save_lease_instance_by_id, load_lease_instance_by_id
};

/// Soroban-compliant Utility NFT structure for lessee access
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LesseeAccessToken {
    pub lease_id: u64,
    pub lessee: Address,
    pub asset_identifier: String, // Off-chain asset ID (IoT device, metaverse server, etc.)
    pub expiration_timestamp: u64,
    pub minted_at: u64,
    pub last_verified: u64,
    pub verification_count: u32,
    pub transferable: bool, // Subleasing permission
    pub transfer_count: u32,
    pub revoked: bool,
    pub revoked_at: Option<u64>,
    pub revocation_reason: Option<RevocationReason>,
    pub asset_type: AssetType,
    pub access_level: AccessLevel,
}

/// Asset type classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetType {
    Physical,     // Physical property (house, apartment, car)
    Digital,      // Digital asset (metaverse server, software license)
    IoT,          // IoT device (smart lock, sensor, camera)
    Hybrid,       // Combined physical-digital asset
}

/// Access level classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    Full,         // Complete access
    Limited,      // Restricted access
    TimeBased,    // Time-limited access
    Conditional,  // Conditional access
}

/// Revocation reason enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevocationReason {
    LeaseTerminated,
    LeaseExpired,
    LeaseEvicted,
    LeaseDefaulted,
    LeaseDisputed,
    SecurityBreach,
    AssetDamage,
    RegulatoryCompliance,
    CourtOrder,
    Other(String),
}

/// Access verification request from external systems
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessVerificationRequest {
    pub token_id: u128,
    pub requesting_system: Address,
    pub verification_purpose: VerificationPurpose,
    pub system_identifier: String,
    pub timestamp: u64,
}

/// Verification purpose for external systems
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationPurpose {
    Unlock,        // Unlock smart lock or door
    Access,        // General access verification
    Authenticate,  // User authentication
    Validate,      // Token validity check
    Audit,         // Audit trail verification
}

/// Access verification response
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessVerificationResponse {
    pub token_id: u128,
    pub is_valid: bool,
    pub lessee: Address,
    pub asset_identifier: String,
    pub expiration_timestamp: u64,
    pub access_level: AccessLevel,
    pub verification_timestamp: u64,
    pub remaining_time: u64,
    pub transferable: bool,
}

/// Token transfer request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenTransferRequest {
    pub token_id: u128,
    pub from_lessee: Address,
    pub to_lessee: Address,
    pub transfer_reason: String,
    pub timestamp: u64,
}

/// Events for access token operations
#[contractevent]
pub struct LesseeAccessGranted {
    pub lease_id: u64,
    pub token_id: u128,
    pub lessee: Address,
    pub asset_identifier: String,
    pub expiration_timestamp: u64,
    pub granted_at: u64,
}

#[contractevent]
pub struct LesseeAccessRevoked {
    pub lease_id: u64,
    pub token_id: u128,
    pub lessee: Address,
    pub revocation_reason: RevocationReason,
    pub revoked_at: u64,
}

#[contractevent]
pub struct AccessTokenTransferred {
    pub token_id: u128,
    pub from_lessee: Address,
    pub to_lessee: Address,
    pub transfer_reason: String,
    pub transferred_at: u64,
}

#[contractevent]
pub struct AccessTokenVerified {
    pub token_id: u128,
    pub requesting_system: Address,
    pub verification_purpose: VerificationPurpose,
    pub is_valid: bool,
    pub verified_at: u64,
}

#[contractevent]
pub struct AccessTokenRenewed {
    pub token_id: u128,
    pub new_expiration: u64,
    pub renewed_at: u64,
}

/// Access token errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessError {
    LeaseNotFound = 5001,
    TokenNotFound = 5002,
    TokenAlreadyExists = 5003,
    InvalidLeaseState = 5004,
    UnauthorizedTransfer = 5005,
    TokenRevoked = 5006,
    TokenExpired = 5007,
    TransferNotAllowed = 5008,
    VerificationFailed = 5009,
    RevocationFailed = 5010,
    RenewalFailed = 5011,
    InvalidAssetType = 5012,
    InvalidAccessLevel = 5013,
    CrossContractCallFailed = 5014,
}

/// Extended DataKey for access token operations
#[contracttype]
#[derive(Debug, Clone)]
pub enum AccessDataKey {
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
    
    // Access token keys
    LesseeAccessToken(u128),
    LeaseToAccessToken(u64),
    LesseeToTokens(Address),
    AssetToTokens(String),
    VerificationCache(u128, Address, u64), // token_id, system, timestamp
    RevocationHistory(u64, u64), // lease_id, revocation_index
}

/// Lessee Access Token Manager
pub struct LesseeAccessTokenManager;

impl LesseeAccessTokenManager {
    /// Mint lessee access token after security deposit is secured
    pub fn mint_lessee_access_token(
        env: Env,
        lease_id: u64,
        asset_identifier: String,
        asset_type: AssetType,
        access_level: AccessLevel,
        transferable: bool,
    ) -> Result<u128, AccessError> {
        // Verify lease exists and is in appropriate state
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(AccessError::LeaseNotFound)?;
        
        // Verify lease is in active state
        if lease.status != LeaseStatus::Active {
            return Err(AccessError::InvalidLeaseState);
        }
        
        // Verify security deposit is secured
        if lease.deposit_status != DepositStatus::Held {
            return Err(AccessError::InvalidLeaseState);
        }
        
        // Check if access token already exists
        if env.storage().persistent().has(&AccessDataKey::LeaseToAccessToken(lease_id)) {
            return Err(AccessError::TokenAlreadyExists);
        }
        
        // Generate unique token ID
        let token_id = Self::generate_unique_token_id(&env, lease_id);
        
        // Create access token
        let token = LesseeAccessToken {
            lease_id,
            lessee: lease.tenant,
            asset_identifier: asset_identifier.clone(),
            expiration_timestamp: lease.end_date,
            minted_at: env.ledger().timestamp(),
            last_verified: env.ledger().timestamp(),
            verification_count: 0,
            transferable,
            transfer_count: 0,
            revoked: false,
            revoked_at: None,
            revocation_reason: None,
            asset_type,
            access_level,
        };
        
        // Store access token
        Self::store_access_token(&env, token_id, &token)?;
        
        // Update lease instance to reference access token
        Self::update_lease_for_access_token(&env, lease_id, token_id)?;
        
        // Emit access granted event
        LesseeAccessGranted {
            lease_id,
            token_id,
            lessee: lease.tenant,
            asset_identifier,
            expiration_timestamp: token.expiration_timestamp,
            granted_at: token.minted_at,
        }.publish(&env);
        
        Ok(token_id)
    }
    
    /// Verify access token for external systems (IoT devices, smart locks, etc.)
    pub fn verify_access_token(
        env: Env,
        request: AccessVerificationRequest,
    ) -> Result<AccessVerificationResponse, AccessError> {
        // Get access token
        let token = env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(request.token_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        let current_time = env.ledger().timestamp();
        
        // Check if token is valid
        let is_valid = Self::is_token_valid(&token, current_time);
        
        // Calculate remaining time
        let remaining_time = if current_time < token.expiration_timestamp {
            token.expiration_timestamp - current_time
        } else {
            0
        };
        
        // Update verification statistics
        Self::update_verification_stats(&env, request.token_id)?;
        
        // Cache verification result
        Self::cache_verification_result(&env, &request, is_valid)?;
        
        // Create response
        let response = AccessVerificationResponse {
            token_id: request.token_id,
            is_valid,
            lessee: token.lessee,
            asset_identifier: token.asset_identifier,
            expiration_timestamp: token.expiration_timestamp,
            access_level: token.access_level,
            verification_timestamp: current_time,
            remaining_time,
            transferable: token.transferable,
        };
        
        // Emit verification event
        AccessTokenVerified {
            token_id: request.token_id,
            requesting_system: request.requesting_system,
            verification_purpose: request.verification_purpose,
            is_valid,
            verified_at: current_time,
        }.publish(&env);
        
        Ok(response)
    }
    
    /// Transfer access token (subleasing)
    pub fn transfer_access_token(
        env: Env,
        request: TokenTransferRequest,
    ) -> Result<(), AccessError> {
        // Get access token
        let mut token = env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(request.token_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        // Verify token is valid and transferable
        if !token.transferable {
            return Err(AccessError::TransferNotAllowed);
        }
        
        if token.revoked {
            return Err(AccessError::TokenRevoked);
        }
        
        if env.ledger().timestamp() > token.expiration_timestamp {
            return Err(AccessError::TokenExpired);
        }
        
        // Verify current lessee
        if token.lessee != request.from_lessee {
            return Err(AccessError::UnauthorizedTransfer);
        }
        
        // Update token
        token.lessee = request.to_lessee.clone();
        token.transfer_count += 1;
        
        // Store updated token
        Self::store_access_token(&env, request.token_id, &token)?;
        
        // Update lessee mappings
        Self::update_lessee_mappings(&env, request.token_id, request.from_lessee, request.to_lessee)?;
        
        // Emit transfer event
        AccessTokenTransferred {
            token_id: request.token_id,
            from_lessee: request.from_lessee,
            to_lessee: request.to_lessee,
            transfer_reason: request.transfer_reason,
            transferred_at: request.timestamp,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Revoke access token immediately upon lease breach
    pub fn revoke_access_token(
        env: Env,
        lease_id: u64,
        revocation_reason: RevocationReason,
    ) -> Result<(), AccessError> {
        // Get token ID from lease
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&AccessDataKey::LeaseToAccessToken(lease_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        // Get access token
        let mut token = env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(token_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        // Update token revocation
        token.revoked = true;
        token.revoked_at = Some(env.ledger().timestamp());
        token.revocation_reason = Some(revocation_reason.clone());
        
        // Store updated token
        Self::store_access_token(&env, token_id, &token)?;
        
        // Store revocation history
        Self::store_revocation_history(&env, lease_id, token_id, &revocation_reason)?;
        
        // Emit revocation event
        LesseeAccessRevoked {
            lease_id,
            token_id,
            lessee: token.lessee,
            revocation_reason,
            revoked_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Renew access token for lease renewals
    pub fn renew_access_token(
        env: Env,
        lease_id: u64,
        new_expiration: u64,
    ) -> Result<(), AccessError> {
        // Get token ID from lease
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&AccessDataKey::LeaseToAccessToken(lease_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        // Get access token
        let mut token = env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(token_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        // Verify token is not revoked
        if token.revoked {
            return Err(AccessError::TokenRevoked);
        }
        
        // Update expiration
        token.expiration_timestamp = new_expiration;
        
        // Store updated token
        Self::store_access_token(&env, token_id, &token)?;
        
        // Emit renewal event
        AccessTokenRenewed {
            token_id,
            new_expiration,
            renewed_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get access token by token ID
    pub fn get_access_token(env: Env, token_id: u128) -> Result<LesseeAccessToken, AccessError> {
        env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(token_id))
            .ok_or(AccessError::TokenNotFound)
    }
    
    /// Get access token by lease ID
    pub fn get_access_token_by_lease(env: Env, lease_id: u64) -> Result<LesseeAccessToken, AccessError> {
        let token_id = env.storage()
            .persistent()
            .get::<_, u128>(&AccessDataKey::LeaseToAccessToken(lease_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        Self::get_access_token(env, token_id)
    }
    
    /// Get all tokens for a lessee
    pub fn get_lessee_tokens(env: Env, lessee: Address) -> Vec<u128> {
        env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::LesseeToTokens(lessee))
            .unwrap_or_else(|| Vec::new(&env))
    }
    
    /// Get all tokens for an asset
    pub fn get_asset_tokens(env: Env, asset_identifier: String) -> Vec<u128> {
        env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::AssetToTokens(asset_identifier))
            .unwrap_or_else(|| Vec::new(&env))
    }
    
    /// Instant revocation hook for lease state changes
    pub fn handle_lease_state_change(
        env: Env,
        lease_id: u64,
        old_status: LeaseStatus,
        new_status: LeaseStatus,
    ) -> Result<(), AccessError> {
        // Determine if revocation is needed
        let revocation_reason = match new_status {
            LeaseStatus::Terminated => Some(RevocationReason::LeaseTerminated),
            LeaseStatus::Expired => Some(RevocationReason::LeaseExpired),
            LeaseStatus::Disputed => Some(RevocationReason::LeaseDisputed),
            _ => None,
        };
        
        // If revocation is needed, revoke the token
        if let Some(reason) = revocation_reason {
            Self::revoke_access_token(env, lease_id, reason)?;
        }
        
        Ok(())
    }
    
    // Helper methods
    
    fn generate_unique_token_id(env: &Env, lease_id: u64) -> u128 {
        // Generate unique token ID based on lease ID and timestamp
        let timestamp = env.ledger().timestamp();
        ((lease_id as u128) << 64) | (timestamp as u128)
    }
    
    fn store_access_token(env: &Env, token_id: u128, token: &LesseeAccessToken) -> Result<(), AccessError> {
        // Store access token
        env.storage()
            .persistent()
            .set(&AccessDataKey::LesseeAccessToken(token_id), token);
        
        // Store lease to token mapping
        env.storage()
            .persistent()
            .set(&AccessDataKey::LeaseToAccessToken(token.lease_id), &token_id);
        
        // Update lessee mappings
        let mut lessee_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::LesseeToTokens(token.lessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        lessee_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&AccessDataKey::LesseeToTokens(token.lessee.clone()), &lessee_tokens);
        
        // Update asset mappings
        let mut asset_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::AssetToTokens(token.asset_identifier.clone()))
            .unwrap_or_else(|| Vec::new(env));
        asset_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&AccessDataKey::AssetToTokens(token.asset_identifier.clone()), &asset_tokens);
        
        // Set TTL for all entries
        let ttl = token.expiration_timestamp - env.ledger().timestamp();
        let key = AccessDataKey::LesseeAccessToken(token_id);
        env.storage()
            .persistent()
            .extend_ttl(&key, ttl, ttl);
        
        Ok(())
    }
    
    fn update_lease_for_access_token(env: &Env, lease_id: u64, token_id: u128) -> Result<(), AccessError> {
        let mut lease = load_lease_instance_by_id(env, lease_id)
            .ok_or(AccessError::LeaseNotFound)?;
        
        // Update lease to reference access token
        lease.nft_contract = Some(env.current_contract_address());
        lease.token_id = Some(token_id);
        
        save_lease_instance_by_id(env, lease_id, &lease);
        
        Ok(())
    }
    
    fn is_token_valid(token: &LesseeAccessToken, current_time: u64) -> bool {
        !token.revoked && current_time <= token.expiration_timestamp
    }
    
    fn update_verification_stats(env: &Env, token_id: u128) -> Result<(), AccessError> {
        let mut token = env.storage()
            .persistent()
            .get::<_, LesseeAccessToken>(&AccessDataKey::LesseeAccessToken(token_id))
            .ok_or(AccessError::TokenNotFound)?;
        
        token.last_verified = env.ledger().timestamp();
        token.verification_count += 1;
        
        env.storage()
            .persistent()
            .set(&AccessDataKey::LesseeAccessToken(token_id), &token);
        
        Ok(())
    }
    
    fn cache_verification_result(
        env: &Env,
        request: &AccessVerificationRequest,
        is_valid: bool,
    ) -> Result<(), AccessError> {
        // Cache for 5 minutes
        let cache_key = AccessDataKey::VerificationCache(
            request.token_id,
            request.requesting_system.clone(),
            request.timestamp,
        );
        env.storage()
            .temporary()
            .set(&cache_key, &is_valid);
        env.storage()
            .temporary()
            .extend_ttl(&cache_key, 300, 300); // 5 minutes
        
        Ok(())
    }
    
    fn update_lessee_mappings(
        env: &Env,
        token_id: u128,
        from_lessee: Address,
        to_lessee: Address,
    ) -> Result<(), AccessError> {
        // Remove from previous lessee
        let mut from_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::LesseeToTokens(from_lessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        from_tokens.retain(|&id| id != token_id);
        env.storage()
            .persistent()
            .set(&AccessDataKey::LesseeToTokens(from_lessee), &from_tokens);
        
        // Add to new lessee
        let mut to_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&AccessDataKey::LesseeToTokens(to_lessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        to_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&AccessDataKey::LesseeToTokens(to_lessee), &to_tokens);
        
        Ok(())
    }
    
    fn store_revocation_history(
        env: &Env,
        lease_id: u64,
        token_id: u128,
        reason: &RevocationReason,
    ) -> Result<(), AccessError> {
        let revocation_index = env.storage()
            .persistent()
            .get::<_, u64>(&AccessDataKey::TenantFlag(lease_id))
            .unwrap_or(0);
        
        let history_key = AccessDataKey::RevocationHistory(lease_id, revocation_index);
        let revocation_record = (token_id, reason.clone(), env.ledger().timestamp());
        
        env.storage()
            .persistent()
            .set(&history_key, &revocation_record);
        
        // Update revocation index
        env.storage()
            .persistent()
            .set(&AccessDataKey::TenantFlag(lease_id), &(revocation_index + 1));
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_mint_lessee_access_token() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        
        // Create a test lease
        let lease_id = 1u64;
        let lease = LeaseInstance {
            landlord: lessor,
            tenant: lessee,
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp(),
            end_date: env.ledger().timestamp() + (30 * 24 * 60 * 60),
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Active,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: env.ledger().timestamp() + (30 * 24 * 60 * 60),
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: env.ledger().timestamp(),
            deposit_status: DepositStatus::Held,
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
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Mint access token
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "smart_lock_123"),
            AssetType::IoT,
            AccessLevel::Full,
            false, // Not transferable
        ).unwrap();
        
        // Verify token was created
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert_eq!(token.lease_id, lease_id);
        assert_eq!(token.lessee, lessee);
        assert_eq!(token.asset_identifier, String::from_str(&env, "smart_lock_123"));
        assert_eq!(token.asset_type, AssetType::IoT);
        assert_eq!(token.access_level, AccessLevel::Full);
        assert!(!token.transferable);
        assert!(!token.revoked);
        
        // Verify lease was updated
        let updated_lease = load_lease_instance_by_id(&env, lease_id).unwrap();
        assert_eq!(updated_lease.nft_contract, Some(env.current_contract_address()));
        assert_eq!(updated_lease.token_id, Some(token_id));
    }

    #[test]
    fn test_access_token_verification() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        let requesting_system = TestAddress::generate(&env);
        
        // Setup: Create lease and mint access token
        let lease_id = 2u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "door_lock_456"),
            AssetType::Physical,
            AccessLevel::Full,
            true, // Transferable
        ).unwrap();
        
        // Verify access token
        let request = AccessVerificationRequest {
            token_id,
            requesting_system: requesting_system.clone(),
            verification_purpose: VerificationPurpose::Unlock,
            system_identifier: String::from_str(&env, "main_door"),
            timestamp: env.ledger().timestamp(),
        };
        
        let response = LesseeAccessTokenManager::verify_access_token(env.clone(), request).unwrap();
        
        assert!(response.is_valid);
        assert_eq!(response.lessee, lessee);
        assert_eq!(response.asset_identifier, String::from_str(&env, "door_lock_456"));
        assert_eq!(response.access_level, AccessLevel::Full);
        assert!(response.transferable);
        assert!(response.remaining_time > 0);
    }

    #[test]
    fn test_access_token_revocation() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        
        // Setup: Create lease and mint access token
        let lease_id = 3u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "server_789"),
            AssetType::Digital,
            AccessLevel::Limited,
            false,
        ).unwrap();
        
        // Revoke access token
        let revoke_result = LesseeAccessTokenManager::revoke_access_token(
            env.clone(),
            lease_id,
            RevocationReason::LeaseTerminated,
        );
        assert!(revoke_result.is_ok());
        
        // Verify token is revoked
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert!(token.revoked);
        assert_eq!(token.revocation_reason, Some(RevocationReason::LeaseTerminated));
        
        // Verify verification fails
        let request = AccessVerificationRequest {
            token_id,
            requesting_system: TestAddress::generate(&env),
            verification_purpose: VerificationPurpose::Access,
            system_identifier: String::from_str(&env, "test_system"),
            timestamp: env.ledger().timestamp(),
        };
        
        let response = LesseeAccessTokenManager::verify_access_token(env.clone(), request).unwrap();
        assert!(!response.is_valid);
    }

    #[test]
    fn test_access_token_transfer() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Setup: Create lease and mint transferable access token
        let lease_id = 4u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "car_key_101"),
            AssetType::Physical,
            AccessLevel::Full,
            true, // Transferable
        ).unwrap();
        
        // Transfer access token
        let transfer_request = TokenTransferRequest {
            token_id,
            from_lessee: lessee,
            to_lessee: sublessee,
            transfer_reason: String::from_str(&env, "Sublease agreement"),
            timestamp: env.ledger().timestamp(),
        };
        
        let transfer_result = LesseeAccessTokenManager::transfer_access_token(env.clone(), transfer_request);
        assert!(transfer_result.is_ok());
        
        // Verify transfer
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert_eq!(token.lessee, sublessee);
        assert_eq!(token.transfer_count, 1);
        
        // Verify lessee mappings updated
        let lessee_tokens = LesseeAccessTokenManager::get_lessee_tokens(env.clone(), lessee);
        let sublessee_tokens = LesseeAccessTokenManager::get_lessee_tokens(env.clone(), sublessee);
        
        assert!(!lessee_tokens.contains(&token_id));
        assert!(sublessee_tokens.contains(&token_id));
    }

    #[test]
    fn test_access_token_renewal() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        
        // Setup: Create lease and mint access token
        let lease_id = 5u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "vpn_access_202"),
            AssetType::Digital,
            AccessLevel::TimeBased,
            false,
        ).unwrap();
        
        // Get original expiration
        let original_token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        let original_expiration = original_token.expiration_timestamp;
        
        // Renew access token
        let new_expiration = original_expiration + (30 * 24 * 60 * 60); // Add 30 days
        let renewal_result = LesseeAccessTokenManager::renew_access_token(env.clone(), lease_id, new_expiration);
        assert!(renewal_result.is_ok());
        
        // Verify renewal
        let renewed_token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert_eq!(renewed_token.expiration_timestamp, new_expiration);
        assert!(renewed_token.expiration_timestamp > original_expiration);
    }

    #[test]
    fn test_non_transferable_token() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        let sublessee = TestAddress::generate(&env);
        
        // Setup: Create lease and mint non-transferable access token
        let lease_id = 6u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "exclusive_access_303"),
            AssetType::Hybrid,
            AccessLevel::Conditional,
            false, // Not transferable
        ).unwrap();
        
        // Attempt transfer (should fail)
        let transfer_request = TokenTransferRequest {
            token_id,
            from_lessee: lessee,
            to_lessee: sublessee,
            transfer_reason: String::from_str(&env, "Unauthorized transfer"),
            timestamp: env.ledger().timestamp(),
        };
        
        let transfer_result = LesseeAccessTokenManager::transfer_access_token(env.clone(), transfer_request);
        assert_eq!(transfer_result, Err(AccessError::TransferNotAllowed));
        
        // Verify token still belongs to original lessee
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert_eq!(token.lessee, lessee);
        assert_eq!(token.transfer_count, 0);
    }

    #[test]
    fn test_lease_state_change_revocation() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        
        // Setup: Create lease and mint access token
        let lease_id = 7u64;
        let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
            env.clone(),
            lease_id,
            String::from_str(&env, "storage_unit_404"),
            AssetType::Physical,
            AccessLevel::Full,
            true,
        ).unwrap();
        
        // Handle lease state change to terminated
        let state_change_result = LesseeAccessTokenManager::handle_lease_state_change(
            env.clone(),
            lease_id,
            LeaseStatus::Active,
            LeaseStatus::Terminated,
        );
        assert!(state_change_result.is_ok());
        
        // Verify token was revoked
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id).unwrap();
        assert!(token.revoked);
        assert_eq!(token.revocation_reason, Some(RevocationReason::LeaseTerminated));
    }

    #[test]
    fn test_get_tokens_by_lessee_and_asset() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let lessee = TestAddress::generate(&env);
        
        // Create multiple leases and tokens
        for i in 0..3 {
            let lease_id = (i + 10) as u64;
            let token_id = LesseeAccessTokenManager::mint_lessee_access_token(
                env.clone(),
                lease_id,
                String::from_str(&env, &format!("asset_{}", i)),
                AssetType::IoT,
                AccessLevel::Full,
                true,
            ).unwrap();
        }
        
        // Get lessee tokens
        let lessee_tokens = LesseeAccessTokenManager::get_lessee_tokens(env.clone(), lessee);
        assert_eq!(lessee_tokens.len(), 3);
        
        // Get asset tokens
        let asset_tokens = LesseeAccessTokenManager::get_asset_tokens(env.clone(), String::from_str(&env, "asset_1"));
        assert_eq!(asset_tokens.len(), 1);
    }
}
