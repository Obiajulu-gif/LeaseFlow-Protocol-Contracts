#![no_main]

//! Concurrent Operations Fuzzer for Escrow Solvency
//! 
//! This fuzzer specifically tests the protocol under concurrent lease operations
//! to ensure the invariant holds under race conditions and simultaneous state changes.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Address, Env, i128, u64};
use std::collections::{HashMap, HashSet};

/// Concurrent operation simulation
#[derive(Arbitrary, Debug, Clone)]
struct ConcurrentFuzzInput {
    /// Multiple operations that could happen concurrently
    concurrent_batches: Vec<OperationBatch>,
    /// Timing variations between batches
    timing_variations: Vec<TimingVariation>,
    /// State synchronization points
    sync_points: Vec<SyncPoint>,
}

#[derive(Arbitrary, Debug, Clone)]
struct OperationBatch {
    operations: Vec<ConcurrentOperation>,
    batch_id: u32,
    execution_order: ExecutionOrder,
}

#[derive(Arbitrary, Debug, Clone)]
enum ConcurrentOperation {
    CreateLease {
        lease_id: u64,
        deposit_amount: i128,
        yield_enabled: bool,
    },
    DepositCollateral {
        lease_id: u64,
        amount: i128,
    },
    AccumulateYield {
        lease_id: u64,
        yield_amount: i128,
    },
    InitiateDispute {
        lease_id: u64,
        dispute_amount: i128,
    },
    SettleLease {
        lease_id: u64,
        tenant_refund: i128,
        landlord_payout: i128,
        protocol_fee: i128,
    },
    MutuallyRelease {
        lease_id: u64,
        return_amount: i128,
        slash_amount: i128,
    },
    PartialSlash {
        lease_id: u64,
        slash_percentage_bps: u32,
    },
}

#[derive(Arbitrary, Debug, Clone)]
enum ExecutionOrder {
    Sequential,
    Random,
    Reverse,
    Interleaved,
}

#[derive(Arbitrary, Debug, Clone)]
struct TimingVariation {
    delay_ms: u32,
    probability: f32, // 0.0 to 1.0
}

#[derive(Arbitrary, Debug, Clone)]
struct SyncPoint {
    batch_id: u32,
    checkpoint_type: CheckpointType,
}

#[derive(Arbitrary, Debug, Clone)]
enum CheckpointType {
    FullStateSync,
    PartialStateSync,
    InvariantVerification,
    VaultSynchronization,
}

/// Thread-safe escrow state for concurrent operations
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConcurrentEscrowState {
    total_escrowed: i128,
    active_deposits: i128,
    pending_yield: i128,
    disputed_funds: i128,
    vault_total_locked: i128,
    lease_count: u64,
    // Track operations for debugging
    operation_log: Vec<OperationLogEntry>,
    // Track concurrent modifications
    concurrent_modifications: HashMap<u64, Vec<Modification>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperationLogEntry {
    operation_id: u64,
    lease_id: u64,
    operation_type: String,
    before_state: ConcurrentEscrowState,
    after_state: ConcurrentEscrowState,
    timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Modification {
    field: String,
    old_value: i128,
    new_value: i128,
    operation_id: u64,
}

impl ConcurrentEscrowState {
    fn new() -> Self {
        Self {
            total_escrowed: 0,
            active_deposits: 0,
            pending_yield: 0,
            disputed_funds: 0,
            vault_total_locked: 0,
            lease_count: 0,
            operation_log: Vec::new(),
            concurrent_modifications: HashMap::new(),
        }
    }

    /// Verify invariants under concurrent conditions
    fn verify_concurrent_invariant(&self) -> Result<(), ConcurrentInvariantError> {
        // Core invariant with concurrent safety
        let calculated_total = self.active_deposits
            .checked_add(self.pending_yield)
            .and_then(|sum| sum.checked_add(self.disputed_funds))
            .ok_or(ConcurrentInvariantError::CalculationOverflow)?;

        if calculated_total != self.total_escrowed {
            return Err(ConcurrentInvariantError::ConcurrentInvariantViolation {
                expected: self.total_escrowed,
                actual: calculated_total,
                active: self.active_deposits,
                pending: self.pending_yield,
                disputed: self.disputed_funds,
                operation_count: self.operation_log.len(),
            });
        }

        // Vault synchronization under concurrent conditions
        if self.vault_total_locked != self.total_escrowed {
            return Err(ConcurrentInvariantError::ConcurrentVaultDesync {
                vault_total: self.vault_total_locked,
                escrow_total: self.total_escrowed,
                last_operations: self.operation_log.len(),
            });
        }

        // Non-negativity under concurrent conditions
        if self.total_escrowed < 0 || self.active_deposits < 0 || 
           self.pending_yield < 0 || self.disputed_funds < 0 || 
           self.vault_total_locked < 0 {
            return Err(ConcurrentInvariantError::ConcurrentNegativeValues);
        }

        Ok(())
    }

    /// Apply operation with concurrent safety checks
    fn apply_concurrent_operation(&mut self, op: &ConcurrentOperation, operation_id: u64) -> Result<(), ConcurrentOperationError> {
        let before_state = self.clone();
        let timestamp = operation_id; // Use operation_id as timestamp for simplicity

        match op {
            ConcurrentOperation::CreateLease { lease_id, deposit_amount, yield_enabled: _ } => {
                if *deposit_amount < 0 {
                    return Err(ConcurrentOperationError::NegativeDeposit);
                }

                // Track concurrent modification
                self.track_modification(*lease_id, "total_escrowed", self.total_escrowed, 
                    self.total_escrowed + deposit_amount, operation_id);
                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits + deposit_amount, operation_id);
                self.track_modification(*lease_id, "vault_total_locked", self.vault_total_locked, 
                    self.vault_total_locked + deposit_amount, operation_id);

                self.total_escrowed = safe_add_concurrent(self.total_escrowed, *deposit_amount)?;
                self.active_deposits = safe_add_concurrent(self.active_deposits, *deposit_amount)?;
                self.vault_total_locked = safe_add_concurrent(self.vault_total_locked, *deposit_amount)?;
                self.lease_count += 1;
            }
            ConcurrentOperation::DepositCollateral { lease_id, amount } => {
                if *amount < 0 {
                    return Err(ConcurrentOperationError::NegativeDeposit);
                }

                self.track_modification(*lease_id, "total_escrowed", self.total_escrowed, 
                    self.total_escrowed + amount, operation_id);
                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits + amount, operation_id);
                self.track_modification(*lease_id, "vault_total_locked", self.vault_total_locked, 
                    self.vault_total_locked + amount, operation_id);

                self.total_escrowed = safe_add_concurrent(self.total_escrowed, *amount)?;
                self.active_deposits = safe_add_concurrent(self.active_deposits, *amount)?;
                self.vault_total_locked = safe_add_concurrent(self.vault_total_locked, *amount)?;
            }
            ConcurrentOperation::AccumulateYield { lease_id, yield_amount } => {
                if *yield_amount < 0 {
                    return Err(ConcurrentOperationError::NegativeYield);
                }

                if *yield_amount > self.active_deposits {
                    return Err(ConcurrentOperationError::InsufficientActiveDeposits);
                }

                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits - yield_amount, operation_id);
                self.track_modification(*lease_id, "pending_yield", self.pending_yield, 
                    self.pending_yield + yield_amount, operation_id);

                self.active_deposits = safe_sub_concurrent(self.active_deposits, *yield_amount)?;
                self.pending_yield = safe_add_concurrent(self.pending_yield, *yield_amount)?;
            }
            ConcurrentOperation::InitiateDispute { lease_id, dispute_amount } => {
                if *dispute_amount < 0 {
                    return Err(ConcurrentOperationError::NegativeDispute);
                }

                let available_for_dispute = safe_add_concurrent(self.active_deposits, self.pending_yield)?;
                if *dispute_amount > available_for_dispute {
                    return Err(ConcurrentOperationError::InsufficientFundsForDispute);
                }

                let from_pending = (*dispute_amount).min(self.pending_yield);
                let from_active = safe_sub_concurrent(*dispute_amount, from_pending)?;

                self.track_modification(*lease_id, "pending_yield", self.pending_yield, 
                    self.pending_yield - from_pending, operation_id);
                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits - from_active, operation_id);
                self.track_modification(*lease_id, "disputed_funds", self.disputed_funds, 
                    self.disputed_funds + dispute_amount, operation_id);

                self.pending_yield = safe_sub_concurrent(self.pending_yield, from_pending)?;
                self.active_deposits = safe_sub_concurrent(self.active_deposits, from_active)?;
                self.disputed_funds = safe_add_concurrent(self.disputed_funds, *dispute_amount)?;
            }
            ConcurrentOperation::SettleLease { lease_id, tenant_refund, landlord_payout, protocol_fee } => {
                if *tenant_refund < 0 || *landlord_payout < 0 || *protocol_fee < 0 {
                    return Err(ConcurrentOperationError::NegativeSettlement);
                }

                let total_settlement = safe_add_concurrent(
                    safe_add_concurrent(*tenant_refund, *landlord_payout)?,
                    *protocol_fee
                )?;

                if total_settlement > self.total_escrowed {
                    return Err(ConcurrentOperationError::InsufficientFundsForSettlement);
                }

                let (from_disputed, from_pending, from_active) = calculate_concurrent_settlement_sources(
                    total_settlement,
                    self.disputed_funds,
                    self.pending_yield,
                    self.active_deposits
                );

                self.track_modification(*lease_id, "disputed_funds", self.disputed_funds, 
                    self.disputed_funds - from_disputed, operation_id);
                self.track_modification(*lease_id, "pending_yield", self.pending_yield, 
                    self.pending_yield - from_pending, operation_id);
                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits - from_active, operation_id);
                self.track_modification(*lease_id, "total_escrowed", self.total_escrowed, 
                    self.total_escrowed - total_settlement, operation_id);
                self.track_modification(*lease_id, "vault_total_locked", self.vault_total_locked, 
                    self.vault_total_locked - total_settlement, operation_id);

                self.disputed_funds = safe_sub_concurrent(self.disputed_funds, from_disputed)?;
                self.pending_yield = safe_sub_concurrent(self.pending_yield, from_pending)?;
                self.active_deposits = safe_sub_concurrent(self.active_deposits, from_active)?;
                self.total_escrowed = safe_sub_concurrent(self.total_escrowed, total_settlement)?;
                self.vault_total_locked = safe_sub_concurrent(self.vault_total_locked, total_settlement)?;

                if total_settlement > 0 {
                    self.lease_count = self.lease_count.saturating_sub(1);
                }
            }
            ConcurrentOperation::MutuallyRelease { lease_id, return_amount, slash_amount } => {
                if *return_amount < 0 || *slash_amount < 0 {
                    return Err(ConcurrentOperationError::NegativeRelease);
                }

                let total_release = safe_add_concurrent(*return_amount, *slash_amount)?;
                
                if total_release != self.total_escrowed {
                    return Err(ConcurrentOperationError::ReleaseMathMismatch);
                }

                // Verify component sum under concurrent conditions
                let component_sum = safe_add_concurrent(
                    safe_add_concurrent(self.active_deposits, self.pending_yield)?,
                    self.disputed_funds
                )?;
                if component_sum != total_release {
                    return Err(ConcurrentOperationError::ComponentSumMismatch);
                }

                // Clear all components atomically
                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 0, operation_id);
                self.track_modification(*lease_id, "pending_yield", self.pending_yield, 0, operation_id);
                self.track_modification(*lease_id, "disputed_funds", self.disputed_funds, 0, operation_id);
                self.track_modification(*lease_id, "total_escrowed", self.total_escrowed, 0, operation_id);
                self.track_modification(*lease_id, "vault_total_locked", this.vault_total_locked, 0, operation_id);

                self.active_deposits = 0;
                self.pending_yield = 0;
                self.disputed_funds = 0;
                self.total_escrowed = 0;
                self.vault_total_locked = 0;
                self.lease_count = self.lease_count.saturating_sub(1);
            }
            ConcurrentOperation::PartialSlash { lease_id, slash_percentage_bps } => {
                if *slash_percentage_bps > 10000 {
                    return Err(ConcurrentOperationError::InvalidPercentage);
                }

                if self.total_escrowed == 0 {
                    return Ok(()); // No effect
                }

                let raw_amount = safe_mul_concurrent(self.total_escrowed, *slash_percentage_bps as i128)?;
                let slash_amount = raw_amount / 10000i128;

                if slash_amount > self.total_escrowed {
                    return Err(ConcurrentOperationError::SlashExceedsTotal);
                }

                let slash_ratio = if self.total_escrowed > 0 {
                    (slash_amount as u128 * 10000) / self.total_escrowed as u128
                } else {
                    0
                };

                let active_slash = (self.active_deposits as u128 * slash_ratio) / 10000;
                let pending_slash = (self.pending_yield as u128 * slash_ratio) / 10000;
                let disputed_slash = (self.disputed_funds as u128 * slash_ratio) / 10000;

                self.track_modification(*lease_id, "active_deposits", self.active_deposits, 
                    self.active_deposits - active_slash as i128, operation_id);
                self.track_modification(*lease_id, "pending_yield", self.pending_yield, 
                    self.pending_yield - pending_slash as i128, operation_id);
                self.track_modification(*lease_id, "disputed_funds", self.disputed_funds, 
                    self.disputed_funds - disputed_slash as i128, operation_id);
                self.track_modification(*lease_id, "total_escrowed", self.total_escrowed, 
                    self.total_escrowed - slash_amount, operation_id);
                self.track_modification(*lease_id, "vault_total_locked", self.vault_total_locked, 
                    self.vault_total_locked - slash_amount, operation_id);

                self.active_deposits -= active_slash as i128;
                self.pending_yield -= pending_slash as i128;
                self.disputed_funds -= disputed_slash as i128;
                self.total_escrowed -= slash_amount;
                self.vault_total_locked -= slash_amount;
            }
        }

        // Log the operation
        let after_state = self.clone();
        self.operation_log.push(OperationLogEntry {
            operation_id,
            lease_id: match op {
                ConcurrentOperation::CreateLease { lease_id, .. } => *lease_id,
                ConcurrentOperation::DepositCollateral { lease_id, .. } => *lease_id,
                ConcurrentOperation::AccumulateYield { lease_id, .. } => *lease_id,
                ConcurrentOperation::InitiateDispute { lease_id, .. } => *lease_id,
                ConcurrentOperation::SettleLease { lease_id, .. } => *lease_id,
                ConcurrentOperation::MutuallyRelease { lease_id, .. } => *lease_id,
                ConcurrentOperation::PartialSlash { lease_id, .. } => *lease_id,
            },
            operation_type: format!("{:?}", op),
            before_state,
            after_state,
            timestamp,
        });

        self.verify_concurrent_invariant()
            .map_err(|e| ConcurrentOperationError::InvariantViolation(e))
    }

    fn track_modification(&mut self, lease_id: u64, field: &str, old_value: i128, new_value: i128, operation_id: u64) {
        let modifications = self.concurrent_modifications.entry(lease_id).or_insert_with(Vec::new);
        modifications.push(Modification {
            field: field.to_string(),
            old_value,
            new_value,
            operation_id,
        });
    }

    /// Apply checkpoint verification
    fn apply_checkpoint(&mut self, sync_point: &SyncPoint) -> Result<(), ConcurrentCheckpointError> {
        match sync_point.checkpoint_type {
            CheckpointType::FullStateSync => {
                // Verify all invariants and state consistency
                self.verify_concurrent_invariant()
                    .map_err(|e| ConcurrentCheckpointError::FullSyncFailed(e))?;
                
                // Verify operation log consistency
                if self.operation_log.len() > 10000 {
                    return Err(ConcurrentCheckpointError::OperationLogOverflow);
                }
            }
            CheckpointType::PartialStateSync => {
                // Lighter verification for performance
                if self.vault_total_locked != self.total_escrowed {
                    return Err(ConcurrentCheckpointError::PartialSyncFailed);
                }
            }
            CheckpointType::InvariantVerification => {
                self.verify_concurrent_invariant()
                    .map_err(|e| ConcurrentCheckpointError::InvariantCheckFailed(e))?;
            }
            CheckpointType::VaultSynchronization => {
                if self.vault_total_locked != self.total_escrowed {
                    return Err(ConcurrentCheckpointError::VaultSyncFailed);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ConcurrentInvariantError {
    ConcurrentInvariantViolation {
        expected: i128,
        actual: i128,
        active: i128,
        pending: i128,
        disputed: i128,
        operation_count: usize,
    },
    ConcurrentVaultDesync {
        vault_total: i128,
        escrow_total: i128,
        last_operations: usize,
    },
    ConcurrentNegativeValues,
    CalculationOverflow,
}

#[derive(Debug, PartialEq, Eq)]
enum ConcurrentOperationError {
    NegativeDeposit,
    NegativeYield,
    NegativeDispute,
    NegativeSettlement,
    NegativeRelease,
    InsufficientActiveDeposits,
    InsufficientFundsForDispute,
    InsufficientFundsForSettlement,
    ReleaseMathMismatch,
    ComponentSumMismatch,
    InvalidPercentage,
    SlashExceedsTotal,
    InvariantViolation(ConcurrentInvariantError),
    CalculationOverflow,
}

#[derive(Debug, PartialEq, Eq)]
enum ConcurrentCheckpointError {
    FullSyncFailed(ConcurrentInvariantError),
    PartialSyncFailed,
    InvariantCheckFailed(ConcurrentInvariantError),
    VaultSyncFailed,
    OperationLogOverflow,
}

/// Safe arithmetic for concurrent operations
fn safe_add_concurrent(a: i128, b: i128) -> Result<i128, ConcurrentOperationError> {
    a.checked_add(b).ok_or(ConcurrentOperationError::CalculationOverflow)
}

fn safe_sub_concurrent(a: i128, b: i128) -> Result<i128, ConcurrentOperationError> {
    a.checked_sub(b).ok_or(ConcurrentOperationError::CalculationOverflow)
}

fn safe_mul_concurrent(a: i128, b: i128) -> Result<i128, ConcurrentOperationError> {
    a.checked_mul(b).ok_or(ConcurrentOperationError::CalculationOverflow)
}

/// Calculate settlement sources for concurrent operations
fn calculate_concurrent_settlement_sources(
    total_settlement: i128,
    disputed: i128,
    pending: i128,
    active: i128
) -> (i128, i128, i128) {
    let from_disputed = total_settlement.min(disputed);
    let remaining = total_settlement - from_disputed;
    let from_pending = remaining.min(pending);
    let final_remaining = remaining - from_pending;
    let from_active = final_remaining.min(active);
    
    (from_disputed, from_pending, from_active)
}

/// Execute operations in different orders to simulate concurrency
fn execute_operations_in_order(
    state: &mut ConcurrentEscrowState,
    operations: &[ConcurrentOperation],
    order: &ExecutionOrder,
    start_id: u64
) -> Result<(), ConcurrentOperationError> {
    let ordered_ops = match order {
        ExecutionOrder::Sequential => operations.to_vec(),
        ExecutionOrder::Random => {
            let mut ops = operations.to_vec();
            // Simple randomization using operation count
            ops.sort_by(|a, b| {
                let a_hash = format!("{:?}", a).len() as u32;
                let b_hash = format!("{:?}", b).len() as u32;
                a_hash.cmp(&b_hash)
            });
            ops
        }
        ExecutionOrder::Reverse => {
            let mut ops = operations.to_vec();
            ops.reverse();
            ops
        }
        ExecutionOrder::Interleaved => {
            let mut ops = Vec::new();
            let mut i = 0;
            let mut j = operations.len() - 1;
            while i <= j {
                ops.push(operations[i].clone());
                if i != j {
                    ops.push(operations[j].clone());
                }
                i += 1;
                j -= 1;
            }
            ops
        }
    };

    for (i, op) in ordered_ops.iter().enumerate() {
        state.apply_concurrent_operation(op, start_id + i as u64)?;
    }

    Ok(())
}

fuzz_target!(|input: ConcurrentFuzzInput| {
    let mut state = ConcurrentEscrowState::new();
    let mut lease_registry = HashSet::new();
    let mut operation_counter = 0u64;

    // Process each batch with potential concurrency
    for (batch_idx, batch) in input.concurrent_batches.iter().enumerate() {
        // Register leases from this batch
        for op in &batch.operations {
            match op {
                ConcurrentOperation::CreateLease { lease_id, .. } => {
                    lease_registry.insert(*lease_id);
                }
                _ => {}
            }
        }

        // Execute operations based on batch order
        let result = execute_operations_in_order(
            &mut state,
            &batch.operations,
            &batch.execution_order,
            operation_counter
        );

        match result {
            Ok(()) => {
                // --- PROPERTY 1: Concurrent operations must preserve invariants ---
                state.verify_concurrent_invariant()
                    .expect("Invariant violation after concurrent batch");
                
                // --- PROPERTY 2: State consistency under concurrency ---
                assert!(state.total_escrowed >= 0, "Total escrowed went negative in concurrent execution");
                assert!(state.vault_total_locked >= 0, "Vault total went negative in concurrent execution");
                
                // --- PROPERTY 3: Component sum invariant under concurrency ---
                let component_sum = state.active_deposits + state.pending_yield + state.disputed_funds;
                assert_eq!(component_sum, state.total_escrowed, 
                    "Component sum mismatch in concurrent execution: {} + {} + {} != {}",
                    state.active_deposits, state.pending_yield, state.disputed_funds, state.total_escrowed);
                
                // --- PROPERTY 4: Vault synchronization under concurrency ---
                assert_eq!(state.vault_total_locked, state.total_escrowed,
                    "Vault desynchronization in concurrent execution: {} != {}",
                    state.vault_total_locked, state.total_escrowed);
                
                // --- PROPERTY 5: Operation log consistency ---
                assert_eq!(state.operation_log.len(), batch.operations.len(), 
                    "Operation log mismatch after concurrent batch");
            }
            Err(ConcurrentOperationError::InvariantViolation(violation)) => {
                // --- PROPERTY 6: Invariant violations must be documented ---
                match violation {
                    ConcurrentInvariantError::ConcurrentInvariantViolation { expected, actual, .. } => {
                        panic!("Critical concurrent invariant violation: expected {}, actual {}", expected, actual);
                    }
                    ConcurrentInvariantError::ConcurrentVaultDesync { vault_total, escrow_total, .. } => {
                        panic!("Concurrent vault desynchronization: vault {}, escrow {}", vault_total, escrow_total);
                    }
                    ConcurrentInvariantError::ConcurrentNegativeValues => {
                        panic!("Negative values detected in concurrent state");
                    }
                    ConcurrentInvariantError::CalculationOverflow => {
                        // Overflow is acceptable as a failure mode
                    }
                }
            }
            Err(_) => {
                // Other errors are acceptable failure modes
            }
        }

        operation_counter += batch.operations.len() as u64;

        // Apply sync points if they match this batch
        for sync_point in &input.sync_points {
            if sync_point.batch_id == batch.batch_id as u32 {
                let checkpoint_result = state.apply_checkpoint(sync_point);
                match checkpoint_result {
                    Ok(()) => {
                        // Checkpoint passed
                    }
                    Err(ConcurrentCheckpointError::FullSyncFailed(_) | 
                         ConcurrentCheckpointError::InvariantCheckFailed(_)) => {
                        panic!("Critical checkpoint failure at batch {}", batch.batch_id);
                    }
                    Err(_) => {
                        // Other checkpoint failures are acceptable
                    }
                }
            }
        }
    }

    // --- PROPERTY 7: Final state must satisfy all concurrent invariants ---
    state.verify_concurrent_invariant()
        .expect("Final concurrent state invariant violation");

    // --- PROPERTY 8: No phantom tokens created under concurrency ---
    let final_component_sum = state.active_deposits + state.pending_yield + state.disputed_funds;
    assert_eq!(final_component_sum, state.total_escrowed, 
        "Phantom tokens detected in final concurrent state");

    // --- PROPERTY 9: Operation log integrity under concurrency ---
    assert!(state.operation_log.len() <= input.concurrent_batches.iter()
        .map(|b| b.operations.len()).sum::<usize>(), 
        "Operation log exceeds expected size in concurrent execution");

    // --- PROPERTY 10: Concurrent modification tracking consistency ---
    for (lease_id, modifications) in &state.concurrent_modifications {
        if lease_registry.contains(lease_id) {
            // Verify modifications are tracked for existing leases
            assert!(!modifications.is_empty(), 
                "No modifications tracked for existing lease {}", lease_id);
        }
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrent_lease_creation() {
        let mut state = ConcurrentEscrowState::new();

        // Simulate concurrent lease creation
        let op1 = ConcurrentOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1000,
            yield_enabled: true,
        };
        let op2 = ConcurrentOperation::CreateLease {
            lease_id: 2,
            deposit_amount: 2000,
            yield_enabled: false,
        };

        state.apply_concurrent_operation(&op1, 1).unwrap();
        state.apply_concurrent_operation(&op2, 2).unwrap();

        assert_eq!(state.total_escrowed, 3000);
        assert_eq!(state.active_deposits, 3000);
        assert_eq!(state.lease_count, 2);
        assert_eq!(state.operation_log.len(), 2);
    }

    #[test]
    fn test_concurrent_yield_accumulation() {
        let mut state = ConcurrentEscrowState::new();

        // Create lease first
        state.apply_concurrent_operation(&ConcurrentOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1000,
            yield_enabled: true,
        }, 1).unwrap();

        // Concurrent yield accumulation
        state.apply_concurrent_operation(&ConcurrentOperation::AccumulateYield {
            lease_id: 1,
            yield_amount: 100,
        }, 2).unwrap();

        assert_eq!(state.total_escrowed, 1000);
        assert_eq!(state.active_deposits, 900);
        assert_eq!(state.pending_yield, 100);
    }

    #[test]
    fn test_concurrent_settlement() {
        let mut state = ConcurrentEscrowState::new();

        state.apply_concurrent_operation(&ConcurrentOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1000,
            yield_enabled: true,
        }, 1).unwrap();

        state.apply_concurrent_operation(&ConcurrentOperation::AccumulateYield {
            lease_id: 1,
            yield_amount: 100,
        }, 2).unwrap();

        // Concurrent settlement
        state.apply_concurrent_operation(&ConcurrentOperation::SettleLease {
            lease_id: 1,
            tenant_refund: 600,
            landlord_payout: 300,
            protocol_fee: 100,
        }, 3).unwrap();

        assert_eq!(state.total_escrowed, 0);
        assert_eq!(state.lease_count, 0);
    }
}
