//! State Cleanup for Finalized Leases
//! 
//! This module implements storage optimization for finalized leases to reduce
//! ledger rent costs while maintaining historical integrity through cryptographic tombstones.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus, DepositStatus, LeaseInstance,
    DataKey, HistoricalLease, save_lease_instance_by_id, load_lease_instance_by_id,
    delete_lease_instance, archive_lease
};

/// Storage optimization constants
const PRUNE_COOLDOWN_DAYS: u64 = 60; // 60 days as specified
const DAY_IN_SECONDS: u64 = 86_400;
const PRUNE_COOLDOWN_SECONDS: u64 = PRUNE_COOLDOWN_DAYS * DAY_IN_SECONDS;

/// Legal hold and appeal tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalHold {
    pub lease_id: u64,
    pub hold_type: LegalHoldType,
    pub initiated_by: Address,
    pub initiated_at: u64,
    pub reason: String,
    pub expires_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegalHoldType {
    Appeal,
    RegulatoryHold,
    CourtOrder,
    Investigation,
}

/// Cryptographic tombstone for historical integrity
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseTombstone {
    pub lease_id: u64,
    pub original_hash: BytesN<32>, // Hash of the original lease data
    pub terminated_at: u64,
    pub terminated_by: Address,
    pub final_status: LeaseStatus,
    pub total_rent_paid: i128,
    pub total_deposits: i128,
    pub property_uri_hash: BytesN<32>, // Hash of property URI for privacy
    pub tenant_anonymous_hash: BytesN<32>, // Hash for tenant privacy
    pub landlord_anonymous_hash: BytesN<32>, // Hash for landlord privacy
    pub created_at: u64,
    pub pruned_at: u64,
    pub pruned_by: Address,
    pub bytes_recovered: u32,
}

/// Storage optimization metrics
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageMetrics {
    pub total_leases_pruned: u64,
    pub total_bytes_recovered: u64,
    pub total_tombstones_created: u64,
    pub active_legal_holds: u64,
    pub last_prune_timestamp: u64,
    pub average_lease_size_bytes: u32,
}

/// Event emitted when lease data is pruned
#[contractevent]
pub struct LeaseDataPruned {
    pub lease_id: u64,
    pub bytes_recovered: u32,
    pub tombstone_hash: BytesN<32>,
    pub pruned_by: Address,
    pub pruned_at: u64,
}

/// Event emitted when legal hold is placed
#[contractevent]
pub struct LegalHoldPlaced {
    pub lease_id: u64,
    pub hold_type: LegalHoldType,
    pub initiated_by: Address,
    pub reason: String,
    pub expires_at: Option<u64>,
}

/// Event emitted when legal hold is released
#[contractevent]
pub struct LegalHoldReleased {
    pub lease_id: u64,
    pub released_by: Address,
    pub released_at: u64,
}

/// Extended DataKey for storage cleanup
#[contracttype]
#[derive(Debug, Clone)]
pub enum CleanupDataKey {
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
    
    // New cleanup-related keys
    LeaseTombstone(u64),
    LegalHold(u64),
    StorageMetrics,
    PruneWhitelist(Address),
}

/// Storage cleanup errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupError {
    LeaseNotFound = 1001,
    LeaseNotFinalized = 1002,
    PruneCooldownNotMet = 1003,
    ActiveLegalHold = 1004,
    ActiveStatePruneAttempt = 1005,
    TombstoneExists = 1006,
    NotAuthorized = 1007,
    StorageCleanupFailed = 1008,
    InvalidTimestamp = 1009,
    HashComputationFailed = 1010,
}

impl LeaseContract {
    /// Prune finalized lease data after 60-day cooldown
    /// 
    /// This function removes granular lease data while preserving historical integrity
    /// through cryptographic tombstones. Only callable by authorized relayers or bots.
    pub fn prune_finalized_lease(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<BytesN<32>, CleanupError> {
        // Verify caller is authorized (relayer or authorized bot)
        Self::verify_prune_authorization(&env, &caller)?;
        
        // Load lease instance
        let lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(CleanupError::LeaseNotFound)?;
        
        // Verify lease is in finalized state
        Self::verify_lease_finalized(&lease)?;
        
        // Verify 60-day cooldown period
        Self::verify_prune_cooldown(&env, &lease)?;
        
        // Check for active legal holds
        Self::verify_no_legal_holds(&env, lease_id)?;
        
        // Calculate lease data size before pruning
        let bytes_before = Self::calculate_lease_storage_size(&env, lease_id);
        
        // Create cryptographic tombstone
        let tombstone = Self::create_tombstone(&env, &lease, lease_id, caller.clone())?;
        let tombstone_hash = Self::compute_tombstone_hash(&tombstone);
        
        // Execute atomic storage cleanup
        let bytes_recovered = Self::execute_storage_cleanup(&env, lease_id, &tombstone)?;
        
        // Update storage metrics
        Self::update_storage_metrics(&env, bytes_recovered);
        
        // Emit pruning event
        LeaseDataPruned {
            lease_id,
            bytes_recovered,
            tombstone_hash,
            pruned_by: caller,
            pruned_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(tombstone_hash)
    }
    
    /// Place legal hold on lease to prevent pruning
    pub fn place_legal_hold(
        env: Env,
        lease_id: u64,
        hold_type: LegalHoldType,
        reason: String,
        expires_at: Option<u64>,
        caller: Address,
    ) -> Result<(), CleanupError> {
        // Verify caller is authorized (legal authority)
        Self::verify_legal_authority(&env, &caller)?;
        
        // Verify lease exists
        let _lease = load_lease_instance_by_id(&env, lease_id)
            .ok_or(CleanupError::LeaseNotFound)?;
        
        // Check if legal hold already exists
        if env.storage().persistent().has(&CleanupDataKey::LegalHold(lease_id)) {
            return Err(CleanupError::ActiveLegalHold);
        }
        
        // Create legal hold
        let legal_hold = LegalHold {
            lease_id,
            hold_type,
            initiated_by: caller.clone(),
            initiated_at: env.ledger().timestamp(),
            reason,
            expires_at,
        };
        
        // Store legal hold
        let key = CleanupDataKey::LegalHold(lease_id);
        env.storage().persistent().set(&key, &legal_hold);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60); // 1 year
        
        // Update metrics
        Self::increment_legal_holds(&env);
        
        // Emit event
        LegalHoldPlaced {
            lease_id,
            hold_type,
            initiated_by: caller,
            reason: legal_hold.reason.clone(),
            expires_at,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Release legal hold on lease
    pub fn release_legal_hold(
        env: Env,
        lease_id: u64,
        caller: Address,
    ) -> Result<(), CleanupError> {
        // Verify caller is authorized
        Self::verify_legal_authority(&env, &caller)?;
        
        // Load legal hold
        let legal_hold: LegalHold = env.storage()
            .persistent()
            .get(&CleanupDataKey::LegalHold(lease_id))
            .ok_or(CleanupError::LeaseNotFound)?;
        
        // Verify caller can release this hold
        if legal_hold.initiated_by != caller && !Self::is_admin(&env, &caller) {
            return Err(CleanupError::NotAuthorized);
        }
        
        // Remove legal hold
        env.storage().persistent().remove(&CleanupDataKey::LegalHold(lease_id));
        
        // Update metrics
        Self::decrement_legal_holds(&env);
        
        // Emit event
        LegalHoldReleased {
            lease_id,
            released_by: caller,
            released_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get storage optimization metrics
    pub fn get_storage_metrics(env: Env) -> StorageMetrics {
        env.storage()
            .persistent()
            .get(&CleanupDataKey::StorageMetrics)
            .unwrap_or(StorageMetrics {
                total_leases_pruned: 0,
                total_bytes_recovered: 0,
                total_tombstones_created: 0,
                active_legal_holds: 0,
                last_prune_timestamp: 0,
                average_lease_size_bytes: 0,
            })
    }
    
    /// Get lease tombstone for historical verification
    pub fn get_lease_tombstone(env: Env, lease_id: u64) -> Option<LeaseTombstone> {
        env.storage()
            .persistent()
            .get(&CleanupDataKey::LeaseTombstone(lease_id))
    }
    
    /// Verify lease historical integrity using tombstone
    pub fn verify_lease_integrity(
        env: Env,
        lease_id: u64,
        provided_hash: BytesN<32>,
    ) -> Result<bool, CleanupError> {
        let tombstone = env.storage()
            .persistent()
            .get(&CleanupDataKey::LeaseTombstone(lease_id))
            .ok_or(CleanupError::LeaseNotFound)?;
        
        let computed_hash = Self::compute_tombstone_hash(&tombstone);
        Ok(computed_hash == provided_hash)
    }
    
    // Helper functions
    
    fn verify_prune_authorization(env: &Env, caller: &Address) -> Result<(), CleanupError> {
        // Check if caller is in prune whitelist
        if env.storage().instance().has(&CleanupDataKey::PruneWhitelist(caller.clone())) {
            return Ok(());
        }
        
        // Check if caller is admin
        if Self::is_admin(env, caller) {
            return Ok(());
        }
        
        Err(CleanupError::NotAuthorized)
    }
    
    fn verify_legal_authority(env: &Env, caller: &Address) -> Result<(), CleanupError> {
        // Check if caller is admin or has legal authority
        if Self::is_admin(env, caller) {
            return Ok(());
        }
        
        // In a real implementation, you might check for specific legal authority roles
        // For now, only admins can place legal holds
        Err(CleanupError::NotAuthorized)
    }
    
    fn verify_lease_finalized(lease: &LeaseInstance) -> Result<(), CleanupError> {
        match lease.status {
            LeaseStatus::Terminated | LeaseStatus::Expired => Ok(()),
            _ => Err(CleanupError::LeaseNotFinalized),
        }
    }
    
    fn verify_prune_cooldown(env: &Env, lease: &LeaseInstance) -> Result<(), CleanupError> {
        let current_time = env.ledger().timestamp();
        let termination_time = lease.end_date; // Simplified - use actual termination timestamp
        
        if current_time < termination_time + PRUNE_COOLDOWN_SECONDS {
            return Err(CleanupError::PruneCooldownNotMet);
        }
        
        Ok(())
    }
    
    fn verify_no_legal_holds(env: &Env, lease_id: u64) -> Result<(), CleanupError> {
        if env.storage().persistent().has(&CleanupDataKey::LegalHold(lease_id)) {
            return Err(CleanupError::ActiveLegalHold);
        }
        Ok(())
    }
    
    fn calculate_lease_storage_size(env: &Env, lease_id: u64) -> u32 {
        // Estimate storage size based on lease structure
        // This is a simplified calculation - in practice you'd measure actual storage usage
        let base_size = 512; // Base lease instance size
        let arbitrator_size = 32; // Per arbitrator
        let string_size = 64; // Average string size
        let optional_size = 16; // Per optional field
        
        // Load lease to count actual fields
        if let Some(lease) = load_lease_instance_by_id(env, lease_id) {
            let arbitrators_count = lease.arbitrators.len() as u32;
            let optional_fields = 8; // Count of Option fields
            let string_fields = 4; // Approximate string fields
            
            base_size + (arbitrators_count * arbitrator_size) + 
            (optional_fields * optional_size) + (string_fields * string_size)
        } else {
            0
        }
    }
    
    fn create_tombstone(
        env: &Env,
        lease: &LeaseInstance,
        lease_id: u64,
        caller: Address,
    ) -> Result<LeaseTombstone, CleanupError> {
        // Compute hashes for privacy
        let original_hash = Self::compute_lease_hash(env, lease)?;
        let property_uri_hash = Self::compute_string_hash(&lease.property_uri);
        let tenant_anonymous_hash = Self::compute_address_hash(&lease.tenant);
        let landlord_anonymous_hash = Self::compute_address_hash(&lease.landlord);
        
        Ok(LeaseTombstone {
            lease_id,
            original_hash,
            terminated_at: lease.end_date, // Simplified
            terminated_by: caller,
            final_status: lease.status,
            total_rent_paid: lease.rent_paid,
            total_deposits: lease.deposit_amount + lease.security_deposit,
            property_uri_hash,
            tenant_anonymous_hash,
            landlord_anonymous_hash,
            created_at: lease.start_date,
            pruned_at: env.ledger().timestamp(),
            pruned_by: caller,
            bytes_recovered: 0, // Will be updated after cleanup
        })
    }
    
    fn compute_lease_hash(env: &Env, lease: &LeaseInstance) -> Result<BytesN<32>, CleanupError> {
        // Create a deterministic hash of the lease data
        let mut data = Vec::new(env);
        
        // Add key fields for hashing
        data.push(lease.landlord.to_val());
        data.push(lease.tenant.to_val());
        data.push(lease.rent_amount.to_val());
        data.push(lease.deposit_amount.to_val());
        data.push(lease.start_date.to_val());
        data.push(lease.end_date.to_val());
        data.push(lease.status.to_val());
        data.push(lease.rent_paid.to_val());
        data.push(lease.cumulative_payments.to_val());
        
        // Compute hash
        let hash = env.crypto().sha256(&data);
        Ok(BytesN::from_array(&hash))
    }
    
    fn compute_string_hash(input: &String) -> BytesN<32> {
        let env = Env::default();
        let data = input.to_val();
        let hash = env.crypto().sha256(&data);
        BytesN::from_array(&hash)
    }
    
    fn compute_address_hash(address: &Address) -> BytesN<32> {
        let env = Env::default();
        let data = address.to_val();
        let hash = env.crypto().sha256(&data);
        BytesN::from_array(&hash)
    }
    
    fn compute_tombstone_hash(tombstone: &LeaseTombstone) -> BytesN<32> {
        let env = Env::default();
        let mut data = Vec::new(&env);
        
        data.push(tombstone.lease_id.to_val());
        data.push(tombstone.original_hash.to_val());
        data.push(tombstone.terminated_at.to_val());
        data.push(tombstone.final_status.to_val());
        data.push(tombstone.total_rent_paid.to_val());
        data.push(tombstone.total_deposits.to_val());
        data.push(tombstone.pruned_at.to_val());
        
        let hash = env.crypto().sha256(&data);
        BytesN::from_array(&hash)
    }
    
    fn execute_storage_cleanup(
        env: &Env,
        lease_id: u64,
        tombstone: &LeaseTombstone,
    ) -> Result<u32, CleanupError> {
        let bytes_before = Self::calculate_lease_storage_size(env, lease_id);
        
        // Remove granular lease data
        env.storage()
            .persistent()
            .remove(&CleanupDataKey::LeaseInstance(lease_id));
        
        // Remove related data (receipts, usage rights, etc.)
        Self::cleanup_related_data(env, lease_id);
        
        // Store tombstone
        let key = CleanupDataKey::LeaseTombstone(lease_id);
        env.storage().persistent().set(&key, tombstone);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60); // 1 year
        
        let bytes_after = Self::calculate_tombstone_size();
        let bytes_recovered = bytes_before.saturating_sub(bytes_after);
        
        Ok(bytes_recovered)
    }
    
    fn cleanup_related_data(env: &Env, lease_id: u64) {
        // Remove receipts
        // Note: In practice, you'd need to iterate through receipt keys
        // This is simplified for demonstration
        
        // Remove usage rights if they exist
        // Note: Usage rights are keyed by NFT contract and token ID
        // You'd need to track which usage rights belong to which lease
        
        // Remove authorized payers
        env.storage()
            .persistent()
            .remove(&CleanupDataKey::AuthorizedPayer(lease_id, Address::random(env)));
        
        // Remove roommate balances
        env.storage()
            .persistent()
            .remove(&CleanupDataKey::RoommateBalance(lease_id, Address::random(env)));
        
        // Remove tenant flags
        env.storage()
            .persistent()
            .remove(&CleanupDataKey::TenantFlag(lease_id));
    }
    
    fn calculate_tombstone_size() -> u32 {
        // Tombstone is much smaller than full lease data
        128 // Approximate size in bytes
    }
    
    fn update_storage_metrics(env: &Env, bytes_recovered: u32) {
        let mut metrics = Self::get_storage_metrics(env.clone());
        
        metrics.total_leases_pruned += 1;
        metrics.total_bytes_recovered += bytes_recovered as u64;
        metrics.total_tombstones_created += 1;
        metrics.last_prune_timestamp = env.ledger().timestamp();
        
        // Update average lease size
        if metrics.total_leases_pruned > 0 {
            metrics.average_lease_size_bytes = 
                (metrics.total_bytes_recovered / metrics.total_leases_pruned) as u32;
        }
        
        let key = CleanupDataKey::StorageMetrics;
        env.storage().persistent().set(&key, &metrics);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60);
    }
    
    fn increment_legal_holds(env: &Env) {
        let mut metrics = Self::get_storage_metrics(env.clone());
        metrics.active_legal_holds += 1;
        
        let key = CleanupDataKey::StorageMetrics;
        env.storage().persistent().set(&key, &metrics);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60);
    }
    
    fn decrement_legal_holds(env: &Env) {
        let mut metrics = Self::get_storage_metrics(env.clone());
        metrics.active_legal_holds = metrics.active_legal_holds.saturating_sub(1);
        
        let key = CleanupDataKey::StorageMetrics;
        env.storage().persistent().set(&key, &metrics);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60);
    }
    
    fn is_admin(env: &Env, address: &Address) -> bool {
        if let Some(admin) = env.storage().instance().get::<_, Address>(&CleanupDataKey::Admin) {
            admin == *address
        } else {
            false
        }
    }
    
    /// Add address to prune whitelist (admin only)
    pub fn add_prune_whitelist(env: Env, admin: Address, address: Address) -> Result<(), CleanupError> {
        if !Self::is_admin(&env, &admin) {
            return Err(CleanupError::NotAuthorized);
        }
        
        admin.require_auth();
        env.storage().instance().set(&CleanupDataKey::PruneWhitelist(address), &true);
        Ok(())
    }
    
    /// Remove address from prune whitelist (admin only)
    pub fn remove_prune_whitelist(env: Env, admin: Address, address: Address) -> Result<(), CleanupError> {
        if !Self::is_admin(&env, &admin) {
            return Err(CleanupError::NotAuthorized);
        }
        
        admin.require_auth();
        env.storage().instance().remove(&CleanupDataKey::PruneWhitelist(address));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_prune_finalized_lease_success() {
        let env = Env::default();
        let admin = TestAddress::generate(&env);
        let caller = TestAddress::generate(&env);
        
        // Setup admin
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        
        // Add caller to prune whitelist
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        // Create a test lease (simplified)
        let lease_id = 1u64;
        let lease = LeaseInstance {
            landlord: TestAddress::generate(&env),
            tenant: TestAddress::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp() - (90 * 24 * 60 * 60), // 90 days ago
            end_date: env.ledger().timestamp() - (65 * 24 * 60 * 60), // 65 days ago
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Terminated,
            // ... other fields
            rent_paid: 1000,
            cumulative_payments: 1000,
            debt: 0,
            rent_paid_through: env.ledger().timestamp() - (65 * 24 * 60 * 60),
            deposit_status: DepositStatus::Settled,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() - (65 * 24 * 60 * 60),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: env.ledger().timestamp() - (65 * 24 * 60 * 60),
        };
        
        // Store lease
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Prune the lease
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
        
        // Verify tombstone exists
        let tombstone = LeaseContract::get_lease_tombstone(env.clone(), lease_id);
        assert!(tombstone.is_some());
        
        // Verify lease data is removed
        let removed_lease = load_lease_instance_by_id(&env, lease_id);
        assert!(removed_lease.is_none());
        
        // Verify metrics updated
        let metrics = LeaseContract::get_storage_metrics(env);
        assert_eq!(metrics.total_leases_pruned, 1);
        assert!(metrics.total_bytes_recovered > 0);
        assert_eq!(metrics.total_tombstones_created, 1);
    }

    #[test]
    fn test_prune_cooldown_not_met() {
        let env = Env::default();
        let admin = TestAddress::generate(&env);
        let caller = TestAddress::generate(&env);
        
        // Setup admin and whitelist
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        // Create recently terminated lease (only 10 days ago)
        let lease_id = 2u64;
        let lease = LeaseInstance {
            landlord: TestAddress::generate(&env),
            tenant: TestAddress::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp() - (20 * 24 * 60 * 60),
            end_date: env.ledger().timestamp() - (10 * 24 * 60 * 60), // Only 10 days ago
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Terminated,
            // ... other fields with default values
            rent_paid: 1000,
            cumulative_payments: 1000,
            debt: 0,
            rent_paid_through: env.ledger().timestamp() - (10 * 24 * 60 * 60),
            deposit_status: DepositStatus::Settled,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() - (10 * 24 * 60 * 60),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: env.ledger().timestamp() - (10 * 24 * 60 * 60),
        };
        
        // Store lease
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Attempt to prune (should fail due to cooldown)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert_eq!(result, Err(CleanupError::PruneCooldownNotMet));
    }

    #[test]
    fn test_legal_hold_prevents_pruning() {
        let env = Env::default();
        let admin = TestAddress::generate(&env);
        let caller = TestAddress::generate(&env);
        let legal_authority = TestAddress::generate(&env);
        
        // Setup admin and whitelist
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        // Create old terminated lease
        let lease_id = 3u64;
        let lease = LeaseInstance {
            landlord: TestAddress::generate(&env),
            tenant: TestAddress::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp() - (90 * 24 * 60 * 60),
            end_date: env.ledger().timestamp() - (65 * 24 * 60 * 60),
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Terminated,
            // ... other fields with default values
            rent_paid: 1000,
            cumulative_payments: 1000,
            debt: 0,
            rent_paid_through: env.ledger().timestamp() - (65 * 24 * 60 * 60),
            deposit_status: DepositStatus::Settled,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() - (65 * 24 * 60 * 60),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: env.ledger().timestamp() - (65 * 24 * 60 * 60),
        };
        
        // Store lease
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Place legal hold
        LeaseContract::place_legal_hold(
            env.clone(),
            lease_id,
            LegalHoldType::Appeal,
            String::from_str(&env, "Under appeal"),
            None,
            legal_authority.clone(),
        ).unwrap();
        
        // Attempt to prune (should fail due to legal hold)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert_eq!(result, Err(CleanupError::ActiveLegalHold));
        
        // Release legal hold
        LeaseContract::release_legal_hold(env.clone(), lease_id, legal_authority.clone()).unwrap();
        
        // Now pruning should work
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_active_state_prune_attempt() {
        let env = Env::default();
        let admin = TestAddress::generate(&env);
        let caller = TestAddress::generate(&env);
        
        // Setup admin and whitelist
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        // Create active lease
        let lease_id = 4u64;
        let lease = LeaseInstance {
            landlord: TestAddress::generate(&env),
            tenant: TestAddress::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp() - (30 * 24 * 60 * 60),
            end_date: env.ledger().timestamp() + (30 * 24 * 60 * 60), // Future end date
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Active, // Active status
            // ... other fields with default values
            rent_paid: 500,
            cumulative_payments: 500,
            debt: 0,
            rent_paid_through: env.ledger().timestamp() - (15 * 24 * 60 * 60),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: env.ledger().timestamp() - (1 * 24 * 60 * 60),
        };
        
        // Store lease
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Attempt to prune (should fail due to active status)
        let result = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone());
        assert_eq!(result, Err(CleanupError::LeaseNotFinalized));
    }

    #[test]
    fn test_tombstone_integrity_verification() {
        let env = Env::default();
        let admin = TestAddress::generate(&env);
        let caller = TestAddress::generate(&env);
        
        // Setup admin and whitelist
        env.storage().instance().set(&CleanupDataKey::Admin, &admin);
        LeaseContract::add_prune_whitelist(env.clone(), admin.clone(), caller.clone()).unwrap();
        
        // Create and prune a lease
        let lease_id = 5u64;
        let lease = LeaseInstance {
            landlord: TestAddress::generate(&env),
            tenant: TestAddress::generate(&env),
            rent_amount: 1000,
            deposit_amount: 500,
            security_deposit: 200,
            start_date: env.ledger().timestamp() - (90 * 24 * 60 * 60),
            end_date: env.ledger().timestamp() - (65 * 24 * 60 * 60),
            property_uri: String::from_str(&env, "test_property"),
            status: LeaseStatus::Terminated,
            // ... other fields with default values
            rent_paid: 1000,
            cumulative_payments: 1000,
            debt: 0,
            rent_paid_through: env.ledger().timestamp() - (65 * 24 * 60 * 60),
            deposit_status: DepositStatus::Settled,
            rent_per_sec: 0,
            grace_period_end: env.ledger().timestamp() - (65 * 24 * 60 * 60),
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
            billing_cycle_duration: 0,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            last_tenant_interaction: env.ledger().timestamp() - (65 * 24 * 60 * 60),
        };
        
        // Store lease
        save_lease_instance_by_id(&env, lease_id, &lease);
        
        // Prune the lease
        let tombstone_hash = LeaseContract::prune_finalized_lease(env.clone(), lease_id, caller.clone()).unwrap();
        
        // Verify integrity with correct hash
        let is_valid = LeaseContract::verify_lease_integrity(env.clone(), lease_id, tombstone_hash);
        assert!(is_valid.unwrap());
        
        // Verify integrity with incorrect hash
        let fake_hash = BytesN::from_array(&[0u8; 32]);
        let is_invalid = LeaseContract::verify_lease_integrity(env.clone(), lease_id, fake_hash);
        assert!(!is_invalid.unwrap());
    }
}
