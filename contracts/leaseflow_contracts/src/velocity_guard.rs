// Velocity Guard Implementation for LeaseFlow Protocol
// Protects against mass termination attacks with rate limiting

use soroban_sdk::{
    contractevent, contracttype, Address, Env, Map, Symbol, Vec, u64, u32,
};
use crate::{LeaseInstance, LeaseError, LeaseStatus, DataKey, load_lease_instance_by_id, save_lease_instance};

// Velocity limit configuration
const VELOCITY_WINDOW: u64 = 86400; // 24 hours in seconds
const VELOCITY_THRESHOLD_PERCENT: u32 = 1000; // 10% in basis points
const MAX_TERMINATIONS_PER_BLOCK: u32 = 500;

// Velocity tracking for lessor portfolio
#[derive(Clone)]
#[contracttype]
pub struct VelocityTracker {
    pub lessor: Address,
    pub total_leases: u64,
    pub terminations_24h: u64,
    pub last_termination_times: Vec<u64>,
    pub is_paused: bool,
    pub pause_timestamp: Option<u64>,
}

// DAO approval request for resuming operations
#[derive(Clone)]
#[contracttype]
pub struct DaoApprovalRequest {
    pub lessor: Address,
    pub request_id: u64,
    pub timestamp: u64,
    pub reason: String,
    pub terminations_count: u64,
    pub approved: bool,
    pub approvals: Vec<Address>,
}

// Events
#[contractevent]
pub struct TerminationVelocityAnomalyDetected {
    pub lessor: Address,
    pub terminations_24h: u64,
    pub portfolio_size: u64,
    pub velocity_percentage: u32,
    pub timestamp: u64,
}

#[contractevent]
pub struct LessorPaused {
    pub lessor: Address,
    pub reason: String,
    pub timestamp: u64,
}

#[contractevent]
pub struct LeaseTerminatedWithVelocityGuard {
    pub lease_id: u64,
    pub lessor: Address,
    pub tenant: Address,
    pub terminated_by: Address,
    pub timestamp: u64,
    pub portfolio_size: u64,
    pub velocity_24h: u64,
}

// Velocity guard implementation
pub struct VelocityGuard;

impl VelocityGuard {
    /// Check velocity limits for a lessor before allowing termination
    pub fn check_velocity_limits(env: &Env, lessor: &Address) -> Result<(), LeaseError> {
        // Check if lessor is paused
        if Self::is_lessor_paused(env, lessor) {
            return Err(LeaseError::VelocityLimitExceeded);
        }
        
        // Get velocity tracker
        let mut tracker = Self::get_velocity_tracker(env, lessor)?;
        
        // Clean old terminations (older than 24 hours)
        Self::cleanup_old_terminations(env, &mut tracker);
        
        // Calculate velocity percentage
        if tracker.total_leases > 0 {
            let velocity_percentage = (tracker.terminations_24h * 10000) / tracker.total_leases; // basis points
            
            if velocity_percentage > VELOCITY_THRESHOLD_PERCENT {
                // Trigger velocity anomaly
                Self::trigger_velocity_anomaly(env, lessor, &tracker, velocity_percentage)?;
                return Err(LeaseError::VelocityLimitExceeded);
            }
        }
        
        Ok(())
    }
    
    /// Record a termination for velocity tracking
    pub fn record_termination(env: &Env, lessor: &Address, lease_id: u64) -> Result<(), LeaseError> {
        let mut tracker = Self::get_velocity_tracker(env, lessor)?;
        
        // Add termination timestamp
        let current_time = env.ledger().timestamp();
        tracker.last_termination_times.push_back(current_time);
        tracker.terminations_24h += 1;
        
        // Update portfolio
        Self::save_velocity_tracker(env, lessor, &tracker);
        
        Ok(())
    }
    
    /// Initialize velocity tracker for a new lessor
    pub fn initialize_lessor(env: &Env, lessor: &Address) -> Result<(), LeaseError> {
        if Self::has_velocity_tracker(env, lessor) {
            return Ok(());
        }
        
        let tracker = VelocityTracker {
            lessor: lessor.clone(),
            total_leases: 0,
            terminations_24h: 0,
            last_termination_times: Vec::new(env),
            is_paused: false,
            pause_timestamp: None,
        };
        
        Self::save_velocity_tracker(env, lessor, &tracker);
        Ok(())
    }
    
    /// Update lessor portfolio size
    pub fn update_portfolio_size(env: &Env, lessor: &Address, delta: i64) -> Result<(), LeaseError> {
        let mut tracker = Self::get_velocity_tracker(env, lessor)?;
        
        if delta > 0 {
            tracker.total_leases += delta as u64;
        } else if delta < 0 && tracker.total_leases > 0 {
            tracker.total_leases = tracker.total_leases.saturating_sub((-delta) as u64);
        }
        
        Self::save_velocity_tracker(env, lessor, &tracker);
        Ok(())
    }
    
    /// Get velocity tracker for a lessor
    pub fn get_velocity_tracker(env: &Env, lessor: &Address) -> Result<VelocityTracker, LeaseError> {
        let key = DataKey::VelocityTracker(lessor.clone());
        env.storage()
            .instance()
            .get(&key)
            .ok_or(LeaseError::LeaseNotFound)
    }
    
    /// Check if velocity tracker exists
    pub fn has_velocity_tracker(env: &Env, lessor: &Address) -> bool {
        let key = DataKey::VelocityTracker(lessor.clone());
        env.storage().instance().has(&key)
    }
    
    /// Save velocity tracker
    pub fn save_velocity_tracker(env: &Env, lessor: &Address, tracker: &VelocityTracker) {
        let key = DataKey::VelocityTracker(lessor.clone());
        env.storage().instance().set(&key, tracker);
    }
    
    /// Check if lessor is paused
    pub fn is_lessor_paused(env: &Env, lessor: &Address) -> bool {
        let key = DataKey::PausedLessor(lessor.clone());
        env.storage().instance().get(&key).unwrap_or(false)
    }
    
    /// Clean up old termination records (older than 24 hours)
    fn cleanup_old_terminations(env: &Env, tracker: &mut VelocityTracker) {
        let current_time = env.ledger().timestamp();
        let cutoff_time = current_time - VELOCITY_WINDOW;
        
        // Remove old timestamps
        let mut new_termination_times = Vec::new(env);
        let mut count_24h = 0u64;
        
        for timestamp in tracker.last_termination_times.iter() {
            if timestamp >= cutoff_time {
                new_termination_times.push_back(timestamp);
                count_24h += 1;
            }
        }
        
        tracker.last_termination_times = new_termination_times;
        tracker.terminations_24h = count_24h;
    }
    
    /// Trigger velocity anomaly detection
    fn trigger_velocity_anomaly(
        env: &Env,
        lessor: &Address,
        tracker: &VelocityTracker,
        velocity_percentage: u32,
    ) -> Result<(), LeaseError> {
        // Emit anomaly event
        env.events().publish(
            (Symbol::short("VELOCITY_ANOMALY"), lessor.clone()),
            TerminationVelocityAnomalyDetected {
                lessor: lessor.clone(),
                terminations_24h: tracker.terminations_24h,
                portfolio_size: tracker.total_leases,
                velocity_percentage,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        // Create DAO approval request
        let request_id = env.ledger().sequence();
        let approval_request = DaoApprovalRequest {
            lessor: lessor.clone(),
            request_id,
            timestamp: env.ledger().timestamp(),
            reason: String::from_str(env, "Velocity anomaly detected"),
            terminations_count: tracker.terminations_24h,
            approved: false,
            approvals: Vec::new(env),
        };
        
        // Store approval request
        let request_key = DataKey::DaoApprovalRequest(request_id);
        env.storage().instance().set(&request_key, &approval_request);
        
        // Soft pause the lessor
        Self::pause_lessor(env, lessor, String::from_str(env, "Velocity anomaly - pending DAO review"))?;
        
        Ok(())
    }
    
    /// Pause a lessor (soft pause)
    fn pause_lessor(env: &Env, lessor: &Address, reason: String) -> Result<(), LeaseError> {
        let key = DataKey::PausedLessor(lessor.clone());
        env.storage().instance().set(&key, &true);
        
        // Update portfolio tracker
        let mut tracker = Self::get_velocity_tracker(env, lessor)?;
        tracker.is_paused = true;
        tracker.pause_timestamp = Some(env.ledger().timestamp());
        Self::save_velocity_tracker(env, lessor, &tracker);
        
        // Emit pause event
        env.events().publish(
            (Symbol::short("LESSOR_PAUSED"), lessor.clone()),
            LessorPaused {
                lessor: lessor.clone(),
                reason,
                timestamp: env.ledger().timestamp(),
            }
        );
        
        Ok(())
    }
    
    /// DAO approval for resuming operations
    pub fn dao_approve_resume(
        env: &Env,
        dao_member: &Address,
        lessor: &Address,
        request_id: u64,
    ) -> Result<(), LeaseError> {
        // Verify DAO member
        let dao_multisig: Address = env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(LeaseError::Unauthorised)?;
        
        if dao_member != &dao_multisig {
            return Err(LeaseError::Unauthorised);
        }
        
        // Get approval request
        let request_key = DataKey::DaoApprovalRequest(request_id);
        let mut request: DaoApprovalRequest = env.storage()
            .instance()
            .get(&request_key)
            .ok_or(LeaseError::LeaseNotFound)?;
        
        // Add approval
        if !request.approvals.contains(dao_member) {
            request.approvals.push_back(dao_member.clone());
            env.storage().instance().set(&request_key, &request);
        }
        
        // Check if we have sufficient approvals (simplified - requires 1 approval)
        if request.approvals.len() >= 1 {
            // Resume lessor
            Self::resume_lessor(env, lessor)?;
            request.approved = true;
            env.storage().instance().set(&request_key, &request);
        }
        
        Ok(())
    }
    
    /// Resume a lessor's operations
    fn resume_lessor(env: &Env, lessor: &Address) -> Result<(), LeaseError> {
        let key = DataKey::PausedLessor(lessor.clone());
        env.storage().instance().remove(&key);
        
        // Update portfolio tracker
        let mut tracker = Self::get_velocity_tracker(env, lessor)?;
        tracker.is_paused = false;
        tracker.pause_timestamp = None;
        Self::save_velocity_tracker(env, lessor, &tracker);
        
        Ok(())
    }
    
    /// Get velocity statistics for monitoring
    pub fn get_velocity_stats(env: &Env, lessor: &Address) -> Result<(u64, u64, u32, bool), LeaseError> {
        let tracker = Self::get_velocity_tracker(env, lessor)?;
        let velocity_percentage = if tracker.total_leases > 0 {
            (tracker.terminations_24h * 10000) / tracker.total_leases
        } else {
            0
        };
        
        Ok((
            tracker.total_leases,
            tracker.terminations_24h,
            velocity_percentage,
            tracker.is_paused,
        ))
    }
}
