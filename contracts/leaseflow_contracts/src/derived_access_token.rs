//! Hierarchical Sub-Lease Tokenization (Derived NFTs)
//! 
//! This module implements derived access tokens that create a cryptographic hierarchy
//! of access rights for sub-lessees, with strict temporal and spatial dependencies.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32, u128
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, LeaseInstance, DepositStatus,
    save_lease_instance_by_id, load_lease_instance_by_id
};
use crate::lessee_access_token::{
    LesseeAccessToken, AssetType, AccessLevel, RevocationReason,
    AccessDataKey, LesseeAccessTokenManager
};

/// Spatial zone definitions for fractional access
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpatialZone {
    EntireProperty,     // Full property access
    SpecificRoom(String), // Specific room (e.g., "Bedroom 1", "Kitchen")
    Zone(String),       // General zone (e.g., "Living Area", "Basement")
    CustomArea(String), // Custom defined area
}

/// Derived access token with hierarchical references
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAccessToken {
    pub token_id: u128,
    pub parent_token_id: u128,  // Reference to parent token
    pub root_lease_id: u64,      // Root lease ID
    pub sublessee: Address,
    pub spatial_zone: SpatialZone,
    pub expiration_timestamp: u64,
    pub minted_at: u64,
    pub hierarchy_depth: u32,    // Depth in hierarchy (0 = root)
    pub access_level: AccessLevel,
    pub transferable: bool,
    pub transfer_count: u32,
    pub revoked: bool,
    pub revoked_at: Option<u64>,
    pub revocation_reason: Option<RevocationReason>,
    pub child_tokens: Vec<u128>, // Direct children in hierarchy
}

/// Derived token creation request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedTokenRequest {
    pub parent_token_id: u128,
    pub sublessee: Address,
    pub spatial_zone: SpatialZone,
    pub access_level: AccessLevel,
    pub transferable: bool,
    pub expiration_timestamp: Option<u64>, // None = inherit from parent
}

/// Events for derived token operations
#[contractevent]
pub struct DerivedTokenMinted {
    pub token_id: u128,
    pub parent_token_id: u128,
    pub root_lease_id: u64,
    pub sublessee: Address,
    pub spatial_zone: SpatialZone,
    pub expiration_timestamp: u64,
    pub hierarchy_depth: u32,
    pub minted_at: u64,
}

#[contractevent]
pub struct DerivedHierarchyBurned {
    pub root_token_id: u128,
    pub root_lease_id: u64,
    pub burn_reason: RevocationReason,
    pub total_tokens_burned: u32,
    pub hierarchy_depth: u32,
    pub burned_at: u64,
}

#[contractevent]
pub struct DerivedTokenTransferred {
    pub token_id: u128,
    pub from_sublessee: Address,
    pub to_sublessee: Address,
    pub hierarchy_depth: u32,
    pub transferred_at: u64,
}

#[contractevent]
pub struct SpatialZoneUpdated {
    pub token_id: u128,
    pub old_zone: SpatialZone,
    pub new_zone: SpatialZone,
    pub updated_at: u64,
}

/// Derived token errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedTokenError {
    ParentTokenNotFound = 6001,
    ParentTokenExpired = 6002,
    ParentTokenRevoked = 6003,
    InvalidTemporalDependency = 6004,
    HierarchyDepthExceeded = 6005,
    SpatialConflict = 6006,
    UnauthorizedSublease = 6007,
    RecursiveBurnFailed = 6008,
    TokenAlreadyExists = 6009,
    InvalidSpatialZone = 6010,
    CrossContractCallFailed = 6011,
}

/// Extended DataKey for derived token operations
#[contracttype]
#[derive(Debug, Clone)]
pub enum DerivedDataKey {
    DerivedAccessToken(u128),
    ParentToChildren(u128),     // parent_token_id -> Vec<child_token_id>
    LeaseToDerivedTokens(u64),   // lease_id -> Vec<derived_token_id>
    SublesseeToDerivedTokens(Address),
    SpatialZoneTokens(String, u64), // zone_name, lease_id -> Vec<token_id>
    HierarchyMetrics(u64),      // lease_id -> HierarchyMetrics
}

/// Hierarchy metrics for monitoring
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchyMetrics {
    pub total_tokens: u32,
    pub max_depth: u32,
    pub avg_depth: u32,
    pub last_updated: u64,
}

/// Derived Access Token Manager
pub struct DerivedAccessTokenManager;

impl DerivedAccessTokenManager {
    const MAX_HIERARCHY_DEPTH: u32 = 10; // Prevent infinite recursion
    
    /// Mint a derived access token for sub-lessee
    pub fn mint_derived_access_token(
        env: Env,
        request: DerivedTokenRequest,
    ) -> Result<u128, DerivedTokenError> {
        // Verify parent token exists and is valid
        let parent_token = Self::get_parent_token(&env, request.parent_token_id)?;
        
        // Verify parent token is not revoked and not expired
        let current_time = env.ledger().timestamp();
        if parent_token.revoked {
            return Err(DerivedTokenError::ParentTokenRevoked);
        }
        if current_time > parent_token.expiration_timestamp {
            return Err(DerivedTokenError::ParentTokenExpired);
        }
        
        // Verify parent token allows subleasing (transferable)
        if !parent_token.transferable {
            return Err(DerivedTokenError::UnauthorizedSublease);
        }
        
        // Calculate hierarchy depth
        let parent_depth = Self::get_token_hierarchy_depth(&env, request.parent_token_id)?;
        let hierarchy_depth = parent_depth + 1;
        
        // Enforce maximum hierarchy depth
        if hierarchy_depth > Self::MAX_HIERARCHY_DEPTH {
            return Err(DerivedTokenError::HierarchyDepthExceeded);
        }
        
        // Determine expiration timestamp
        let expiration_timestamp = request.expiration_timestamp
            .unwrap_or(parent_token.expiration_timestamp)
            .min(parent_token.expiration_timestamp); // Cannot exceed parent
        
        // Verify temporal dependency
        if expiration_timestamp > parent_token.expiration_timestamp {
            return Err(DerivedTokenError::InvalidTemporalDependency);
        }
        
        // Verify spatial zone validity
        Self::validate_spatial_zone(&env, &request.spatial_zone, parent_token.lease_id)?;
        
        // Generate unique token ID
        let token_id = Self::generate_unique_derived_token_id(&env, request.parent_token_id);
        
        // Get root lease ID
        let root_lease_id = Self::get_root_lease_id(&env, request.parent_token_id)?;
        
        // Create derived token
        let derived_token = DerivedAccessToken {
            token_id,
            parent_token_id: request.parent_token_id,
            root_lease_id,
            sublessee: request.sublessee.clone(),
            spatial_zone: request.spatial_zone.clone(),
            expiration_timestamp,
            minted_at: current_time,
            hierarchy_depth,
            access_level: request.access_level.clone(),
            transferable: request.transferable,
            transfer_count: 0,
            revoked: false,
            revoked_at: None,
            revocation_reason: None,
            child_tokens: Vec::new(&env),
        };
        
        // Store derived token
        Self::store_derived_token(&env, token_id, &derived_token)?;
        
        // Update parent token's child list
        Self::add_child_to_parent(&env, request.parent_token_id, token_id)?;
        
        // Update hierarchy metrics
        Self::update_hierarchy_metrics(&env, root_lease_id)?;
        
        // Emit derived token minted event
        DerivedTokenMinted {
            token_id,
            parent_token_id: request.parent_token_id,
            root_lease_id,
            sublessee: request.sublessee,
            spatial_zone: request.spatial_zone,
            expiration_timestamp,
            hierarchy_depth,
            minted_at: current_time,
        }.publish(&env);
        
        Ok(token_id)
    }
    
    /// Recursively burn all derived tokens in hierarchy
    pub fn recursive_burn_hierarchy(
        env: Env,
        root_token_id: u128,
        burn_reason: RevocationReason,
    ) -> Result<u32, DerivedTokenError> {
        let mut tokens_burned = 0u32;
        let mut max_depth = 0u32;
        
        // Get root token to determine lease ID
        let root_token = Self::get_derived_token(&env, root_token_id)?;
        let root_lease_id = root_token.root_lease_id;
        
        // Start recursive burning
        Self::recursive_burn_internal(
            &env,
            root_token_id,
            &burn_reason,
            &mut tokens_burned,
            &mut max_depth,
        )?;
        
        // Emit hierarchy burned event
        DerivedHierarchyBurned {
            root_token_id,
            root_lease_id,
            burn_reason,
            total_tokens_burned: tokens_burned,
            hierarchy_depth: max_depth,
            burned_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(tokens_burned)
    }
    
    /// Transfer derived token to new sublessee
    pub fn transfer_derived_token(
        env: Env,
        token_id: u128,
        from_sublessee: Address,
        to_sublessee: Address,
        transfer_reason: String,
    ) -> Result<(), DerivedTokenError> {
        // Get derived token
        let mut token = Self::get_derived_token(&env, token_id)?;
        
        // Verify token is valid and transferable
        if !token.transferable {
            return Err(DerivedTokenError::UnauthorizedSublease);
        }
        
        if token.revoked {
            return Err(DerivedTokenError::ParentTokenRevoked);
        }
        
        if env.ledger().timestamp() > token.expiration_timestamp {
            return Err(DerivedTokenError::ParentTokenExpired);
        }
        
        // Verify current sublessee
        if token.sublessee != from_sublessee {
            return Err(DerivedTokenError::UnauthorizedSublease);
        }
        
        // Update token
        token.sublessee = to_sublessee.clone();
        token.transfer_count += 1;
        
        // Store updated token
        Self::store_derived_token(&env, token_id, &token)?;
        
        // Update sublessee mappings
        Self::update_sublessee_mappings(&env, token_id, from_sublessee, to_sublessee)?;
        
        // Emit transfer event
        DerivedTokenTransferred {
            token_id,
            from_sublessee,
            to_sublessee,
            hierarchy_depth: token.hierarchy_depth,
            transferred_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Update spatial zone for derived token
    pub fn update_spatial_zone(
        env: Env,
        token_id: u128,
        new_zone: SpatialZone,
    ) -> Result<(), DerivedTokenError> {
        // Get derived token
        let mut token = Self::get_derived_token(&env, token_id)?;
        
        // Verify token is not revoked
        if token.revoked {
            return Err(DerivedTokenError::ParentTokenRevoked);
        }
        
        // Validate new spatial zone
        Self::validate_spatial_zone(&env, &new_zone, token.root_lease_id)?;
        
        let old_zone = token.spatial_zone.clone();
        token.spatial_zone = new_zone.clone();
        
        // Store updated token
        Self::store_derived_token(&env, token_id, &token)?;
        
        // Update spatial zone mappings
        Self::update_spatial_mappings(&env, token_id, old_zone.clone(), new_zone.clone())?;
        
        // Emit spatial zone update event
        SpatialZoneUpdated {
            token_id,
            old_zone,
            new_zone,
            updated_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get derived token by ID
    pub fn get_derived_token(env: Env, token_id: u128) -> Result<DerivedAccessToken, DerivedTokenError> {
        env.storage()
            .persistent()
            .get::<_, DerivedAccessToken>(&DerivedDataKey::DerivedAccessToken(token_id))
            .ok_or(DerivedTokenError::ParentTokenNotFound)
    }
    
    /// Get all derived tokens for a lease
    pub fn get_lease_derived_tokens(env: Env, lease_id: u64) -> Vec<u128> {
        env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::LeaseToDerivedTokens(lease_id))
            .unwrap_or_else(|| Vec::new(&env))
    }
    
    /// Get all derived tokens for a sublessee
    pub fn get_sublessee_derived_tokens(env: Env, sublessee: Address) -> Vec<u128> {
        env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::SublesseeToDerivedTokens(sublessee))
            .unwrap_or_else(|| Vec::new(&env))
    }
    
    /// Get hierarchy metrics for lease
    pub fn get_hierarchy_metrics(env: Env, lease_id: u64) -> Result<HierarchyMetrics, DerivedTokenError> {
        env.storage()
            .persistent()
            .get::<_, HierarchyMetrics>(&DerivedDataKey::HierarchyMetrics(lease_id))
            .ok_or(DerivedTokenError::ParentTokenNotFound)
    }
    
    /// Check if derived token is valid
    pub fn is_derived_token_valid(env: Env, token_id: u128) -> Result<bool, DerivedTokenError> {
        let token = Self::get_derived_token(env, token_id)?;
        let current_time = env.ledger().timestamp();
        
        Ok(!token.revoked && current_time <= token.expiration_timestamp)
    }
    
    // Helper methods
    
    fn get_parent_token(env: &Env, parent_token_id: u128) -> Result<LesseeAccessToken, DerivedTokenError> {
        LesseeAccessTokenManager::get_access_token(env.clone(), parent_token_id)
            .map_err(|_| DerivedTokenError::ParentTokenNotFound)
    }
    
    fn get_token_hierarchy_depth(env: &Env, token_id: u128) -> Result<u32, DerivedTokenError> {
        let token = Self::get_derived_token(env.clone(), token_id)?;
        Ok(token.hierarchy_depth)
    }
    
    fn get_root_lease_id(env: &Env, parent_token_id: u128) -> Result<u64, DerivedTokenError> {
        // For now, assume parent token is root lessee token
        // In future, this could traverse up the hierarchy
        let parent_token = Self::get_parent_token(env, parent_token_id)?;
        Ok(parent_token.lease_id)
    }
    
    fn generate_unique_derived_token_id(env: &Env, parent_token_id: u128) -> u128 {
        let timestamp = env.ledger().timestamp();
        ((parent_token_id as u128) << 64) | (timestamp as u128)
    }
    
    fn validate_spatial_zone(env: &Env, zone: &SpatialZone, lease_id: u64) -> Result<(), DerivedTokenError> {
        // Basic validation - could be extended with property layout validation
        match zone {
            SpatialZone::SpecificRoom(room_name) => {
                if room_name.is_empty() {
                    return Err(DerivedTokenError::InvalidSpatialZone);
                }
            },
            SpatialZone::Zone(zone_name) => {
                if zone_name.is_empty() {
                    return Err(DerivedTokenError::InvalidSpatialZone);
                }
            },
            SpatialZone::CustomArea(area_name) => {
                if area_name.is_empty() {
                    return Err(DerivedTokenError::InvalidSpatialZone);
                }
            },
            SpatialZone::EntireProperty => {
                // Always valid
            },
        }
        Ok(())
    }
    
    fn store_derived_token(env: &Env, token_id: u128, token: &DerivedAccessToken) -> Result<(), DerivedTokenError> {
        // Store derived token
        env.storage()
            .persistent()
            .set(&DerivedDataKey::DerivedAccessToken(token_id), token);
        
        // Store in lease mapping
        let mut lease_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::LeaseToDerivedTokens(token.root_lease_id))
            .unwrap_or_else(|| Vec::new(env));
        lease_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&DerivedDataKey::LeaseToDerivedTokens(token.root_lease_id), &lease_tokens);
        
        // Store in sublessee mapping
        let mut sublessee_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::SublesseeToDerivedTokens(token.sublessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        sublessee_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&DerivedDataKey::SublesseeToDerivedTokens(token.sublessee.clone()), &sublessee_tokens);
        
        // Store in spatial zone mapping
        let zone_key = Self::get_zone_key(&token.spatial_zone, token.root_lease_id);
        let mut zone_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&zone_key)
            .unwrap_or_else(|| Vec::new(env));
        zone_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&zone_key, &zone_tokens);
        
        // Set TTL based on expiration
        let ttl = token.expiration_timestamp - env.ledger().timestamp();
        let key = DerivedDataKey::DerivedAccessToken(token_id);
        env.storage()
            .persistent()
            .extend_ttl(&key, ttl, ttl);
        
        Ok(())
    }
    
    fn add_child_to_parent(env: &Env, parent_token_id: u128, child_token_id: u128) -> Result<(), DerivedTokenError> {
        // If parent is a derived token, update its child list
        if let Ok(mut parent_token) = Self::get_derived_token(env.clone(), parent_token_id) {
            parent_token.child_tokens.push_back(child_token_id);
            Self::store_derived_token(env, parent_token_id, &parent_token)?;
        }
        
        // Store in parent-to-children mapping
        let mut children = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::ParentToChildren(parent_token_id))
            .unwrap_or_else(|| Vec::new(env));
        children.push_back(child_token_id);
        env.storage()
            .persistent()
            .set(&DerivedDataKey::ParentToChildren(parent_token_id), &children);
        
        Ok(())
    }
    
    fn recursive_burn_internal(
        env: &Env,
        token_id: u128,
        burn_reason: &RevocationReason,
        tokens_burned: &mut u32,
        max_depth: &mut u32,
    ) -> Result<(), DerivedTokenError> {
        // Get current token
        let token = Self::get_derived_token(env.clone(), token_id)?;
        
        // Update max depth
        *max_depth = (*max_depth).max(token.hierarchy_depth);
        
        // Recursively burn children first
        for &child_id in token.child_tokens.iter() {
            Self::recursive_burn_internal(env, child_id, burn_reason, tokens_burned, max_depth)?;
        }
        
        // Burn current token
        let mut burned_token = token;
        burned_token.revoked = true;
        burned_token.revoked_at = Some(env.ledger().timestamp());
        burned_token.revocation_reason = Some(burn_reason.clone());
        
        // Store burned token
        env.storage()
            .persistent()
            .set(&DerivedDataKey::DerivedAccessToken(token_id), &burned_token);
        
        *tokens_burned += 1;
        
        Ok(())
    }
    
    fn update_hierarchy_metrics(env: &Env, lease_id: u64) -> Result<(), DerivedTokenError> {
        let tokens = Self::get_lease_derived_tokens(env.clone(), lease_id);
        
        let mut total_tokens = tokens.len() as u32;
        let mut max_depth = 0u32;
        let mut total_depth = 0u32;
        
        for token_id in tokens.iter() {
            if let Ok(token) = Self::get_derived_token(env.clone(), *token_id) {
                max_depth = max_depth.max(token.hierarchy_depth);
                total_depth += token.hierarchy_depth;
            }
        }
        
        let avg_depth = if total_tokens > 0 { total_depth / total_tokens } else { 0 };
        
        let metrics = HierarchyMetrics {
            total_tokens,
            max_depth,
            avg_depth,
            last_updated: env.ledger().timestamp(),
        };
        
        env.storage()
            .persistent()
            .set(&DerivedDataKey::HierarchyMetrics(lease_id), &metrics);
        
        Ok(())
    }
    
    fn update_sublessee_mappings(
        env: &Env,
        token_id: u128,
        from_sublessee: Address,
        to_sublessee: Address,
    ) -> Result<(), DerivedTokenError> {
        // Remove from previous sublessee
        let mut from_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::SublesseeToDerivedTokens(from_sublessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        from_tokens.retain(|&id| id != token_id);
        env.storage()
            .persistent()
            .set(&DerivedDataKey::SublesseeToDerivedTokens(from_sublessee), &from_tokens);
        
        // Add to new sublessee
        let mut to_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&DerivedDataKey::SublesseeToDerivedTokens(to_sublessee.clone()))
            .unwrap_or_else(|| Vec::new(env));
        to_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&DerivedDataKey::SublesseeToDerivedTokens(to_sublessee), &to_tokens);
        
        Ok(())
    }
    
    fn update_spatial_mappings(
        env: &Env,
        token_id: u128,
        old_zone: SpatialZone,
        new_zone: SpatialZone,
    ) -> Result<(), DerivedTokenError> {
        // Get token to determine lease ID
        let token = Self::get_derived_token(env.clone(), token_id)?;
        
        // Remove from old zone mapping
        let old_zone_key = Self::get_zone_key(&old_zone, token.root_lease_id);
        let mut old_zone_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&old_zone_key)
            .unwrap_or_else(|| Vec::new(env));
        old_zone_tokens.retain(|&id| id != token_id);
        env.storage()
            .persistent()
            .set(&old_zone_key, &old_zone_tokens);
        
        // Add to new zone mapping
        let new_zone_key = Self::get_zone_key(&new_zone, token.root_lease_id);
        let mut new_zone_tokens = env.storage()
            .persistent()
            .get::<_, Vec<u128>>(&new_zone_key)
            .unwrap_or_else(|| Vec::new(env));
        new_zone_tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&new_zone_key, &new_zone_tokens);
        
        Ok(())
    }
    
    fn get_zone_key(zone: &SpatialZone, lease_id: u64) -> DerivedDataKey {
        match zone {
            SpatialZone::EntireProperty => {
                DerivedDataKey::SpatialZoneTokens(String::from_str(&Env::default(), "entire_property"), lease_id)
            },
            SpatialZone::SpecificRoom(room_name) => {
                DerivedDataKey::SpatialZoneTokens(room_name.clone(), lease_id)
            },
            SpatialZone::Zone(zone_name) => {
                DerivedDataKey::SpatialZoneTokens(zone_name.clone(), lease_id)
            },
            SpatialZone::CustomArea(area_name) => {
                DerivedDataKey::SpatialZoneTokens(area_name.clone(), lease_id)
            },
        }
    }
}
