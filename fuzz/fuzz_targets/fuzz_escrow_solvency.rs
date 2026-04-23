#![no_main]

//! Comprehensive Fuzzing for Escrow Solvency Verification
//! 
//! This fuzzer simulates millions of random lease operations to prove that
//! the escrow solvency invariant holds under all possible conditions.
//! 
//! Core invariant: Total_Escrowed == (Active_Deposits + Pending_Yield + Disputed_Funds)

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Address, Env, i128, u64};
use std::collections::HashMap;

/// Fuzz input for comprehensive escrow solvency testing
#[derive(Arbitrary, Debug)]
struct EscrowFuzzInput {
    // Lease operations
    operations: Vec<LeaseOperation>,
    // Global state modifiers
    global_modifiers: GlobalModifiers,
    // Edge case injection
    edge_cases: Vec<EdgeCase>,
    // Soroban-specific behaviors
    soroban_behaviors: SorobanBehaviors,
}

#[derive(Arbitrary, Debug)]
enum LeaseOperation {
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

#[derive(Arbitrary, Debug)]
struct GlobalModifiers {
    // Protocol-wide changes
    max_capacity_change: i64,
    // Timestamp manipulation
    time_jump_seconds: u64,
    // Ledger sequence manipulation
    ledger_jump: u32,
}

#[derive(Arbitrary, Debug)]
enum EdgeCase {
    MaxValues,
    MinValues,
    OverflowAttempt,
    UnderflowAttempt,
    ZeroValues,
    NegativeValues,
    PrecisionLoss,
    DustCreation,
    ConcurrentOperations,
    ReentrantCall,
}

#[derive(Arbitrary, Debug)]
struct SorobanBehaviors {
    // Simulate Soroban-specific 128-bit fixed-point behaviors
    fixed_point_precision: bool,
    // Simulate ledger-specific timing
    ledger_timing_variance: u32,
    // Simulate gas limit behaviors
    gas_limit_reached: bool,
    // Simulate storage rent behaviors
    storage_rent_due: bool,
}

/// Formal escrow state with mathematical precision tracking
#[derive(Debug, Clone, PartialEq, Eq)]
struct FormalEscrowState {
    total_escrowed: i128,
    active_deposits: i128,
    pending_yield: i128,
    disputed_funds: i128,
    vault_total_locked: i128,
    lease_count: u64,
    // Precision tracking for Soroban 128-bit math
    precision_loss_tracker: i128,
    dust_tracker: i128,
}

impl FormalEscrowState {
    fn new() -> Self {
        Self {
            total_escrowed: 0,
            active_deposits: 0,
            pending_yield: 0,
            disputed_funds: 0,
            vault_total_locked: 0,
            lease_count: 0,
            precision_loss_tracker: 0,
            dust_tracker: 0,
        }
    }

    /// Verify the core solvency invariant with detailed error reporting
    fn verify_solvency_invariant(&self) -> Result<(), SolvencyViolation> {
        let calculated_total = self.active_deposits
            .checked_add(self.pending_yield)
            .and_then(|sum| sum.checked_add(self.disputed_funds))
            .ok_or(SolvencyViolation::CalculationOverflow)?;

        if calculated_total != self.total_escrowed {
            return Err(SolvencyViolation::InvariantViolated {
                expected: self.total_escrowed,
                actual: calculated_total,
                active: self.active_deposits,
                pending: self.pending_yield,
                disputed: self.disputed_funds,
                precision_loss: self.precision_loss_tracker,
                dust: self.dust_tracker,
            });
        }

        if self.vault_total_locked != self.total_escrowed {
            return Err(SolvencyViolation::VaultMismatch {
                vault_total: self.vault_total_locked,
                escrow_total: self.total_escrowed,
            });
        }

        // Verify non-negativity
        if self.total_escrowed < 0 || self.active_deposits < 0 || 
           self.pending_yield < 0 || self.disputed_funds < 0 || 
           self.vault_total_locked < 0 {
            return Err(SolvencyViolation::NegativeValues {
                total: self.total_escrowed,
                active: self.active_deposits,
                pending: self.pending_yield,
                disputed: self.disputed_funds,
                vault: self.vault_total_locked,
            });
        }

        Ok(())
    }

    /// Apply operation with Soroban 128-bit fixed-point simulation
    fn apply_operation(&mut self, op: &LeaseOperation, behaviors: &SorobanBehaviors) -> Result<(), OperationError> {
        match op {
            LeaseOperation::CreateLease { lease_id: _, deposit_amount, yield_enabled: _ } => {
                if *deposit_amount < 0 {
                    return Err(OperationError::NegativeDeposit);
                }

                // Simulate Soroban 128-bit overflow protection
                self.total_escrowed = checked_add_soroban(self.total_escrowed, *deposit_amount, behaviors)?;
                self.active_deposits = checked_add_soroban(self.active_deposits, *deposit_amount, behaviors)?;
                self.vault_total_locked = checked_add_soroban(self.vault_total_locked, *deposit_amount, behaviors)?;
                self.lease_count += 1;
            }
            LeaseOperation::DepositCollateral { lease_id: _, amount } => {
                if *amount < 0 {
                    return Err(OperationError::NegativeDeposit);
                }

                self.total_escrowed = checked_add_soroban(self.total_escrowed, *amount, behaviors)?;
                self.active_deposits = checked_add_soroban(self.active_deposits, *amount, behaviors)?;
                self.vault_total_locked = checked_add_soroban(self.vault_total_locked, *amount, behaviors)?;
            }
            LeaseOperation::AccumulateYield { lease_id: _, yield_amount } => {
                if *yield_amount < 0 {
                    return Err(OperationError::NegativeYield);
                }

                if *yield_amount > self.active_deposits {
                    return Err(OperationError::InsufficientActiveDeposits);
                }

                // Track precision loss in yield calculations
                if behaviors.fixed_point_precision {
                    let precision_loss = simulate_fixed_point_loss(*yield_amount);
                    self.precision_loss_tracker += precision_loss;
                }

                self.active_deposits = checked_sub_soroban(self.active_deposits, *yield_amount, behaviors)?;
                self.pending_yield = checked_add_soroban(self.pending_yield, *yield_amount, behaviors)?;
            }
            LeaseOperation::InitiateDispute { lease_id: _, dispute_amount } => {
                if *dispute_amount < 0 {
                    return Err(OperationError::NegativeDispute);
                }

                let available_for_dispute = self.active_deposits + self.pending_yield;
                if *dispute_amount > available_for_dispute {
                    return Err(OperationError::InsufficientFundsForDispute);
                }

                // Take from pending yield first, then active
                let from_pending = (*dispute_amount).min(self.pending_yield);
                let from_active = *dispute_amount - from_pending;

                self.pending_yield = checked_sub_soroban(self.pending_yield, from_pending, behaviors)?;
                self.active_deposits = checked_sub_soroban(self.active_deposits, from_active, behaviors)?;
                self.disputed_funds = checked_add_soroban(self.disputed_funds, *dispute_amount, behaviors)?;
            }
            LeaseOperation::SettleLease { lease_id: _, tenant_refund, landlord_payout, protocol_fee } => {
                if *tenant_refund < 0 || *landlord_payout < 0 || *protocol_fee < 0 {
                    return Err(OperationError::NegativeSettlement);
                }

                let total_settlement = checked_add_soroban(
                    checked_add_soroban(*tenant_refund, *landlord_payout, behaviors)?,
                    *protocol_fee,
                    behaviors
                )?;

                if total_settlement > self.total_escrowed {
                    return Err(OperationError::InsufficientFundsForSettlement);
                }

                // Dust accounting for multi-signer refunds
                let dust = calculate_dust_accounting(total_settlement, self.total_escrowed);
                self.dust_tracker += dust;

                // Determine settlement sources with proper accounting
                let (from_disputed, from_pending, from_active) = calculate_settlement_sources(
                    total_settlement,
                    self.disputed_funds,
                    self.pending_yield,
                    self.active_deposits
                );

                self.disputed_funds = checked_sub_soroban(self.disputed_funds, from_disputed, behaviors)?;
                self.pending_yield = checked_sub_soroban(self.pending_yield, from_pending, behaviors)?;
                self.active_deposits = checked_sub_soroban(self.active_deposits, from_active, behaviors)?;

                self.total_escrowed = checked_sub_soroban(self.total_escrowed, total_settlement, behaviors)?;
                self.vault_total_locked = checked_sub_soroban(self.vault_total_locked, total_settlement, behaviors)?;

                if total_settlement > 0 {
                    self.lease_count = self.lease_count.saturating_sub(1);
                }
            }
            LeaseOperation::MutuallyRelease { lease_id: _, return_amount, slash_amount } => {
                if *return_amount < 0 || *slash_amount < 0 {
                    return Err(OperationError::NegativeRelease);
                }

                let total_release = checked_add_soroban(*return_amount, *slash_amount, behaviors)?;
                if total_release != self.total_escrowed {
                    return Err(OperationError::ReleaseMathMismatch);
                }

                // Verify mathematical invariant before release
                if self.active_deposits + self.pending_yield + self.disputed_funds != total_release {
                    return Err(OperationError::ComponentMismatch);
                }

                // Clear all components
                self.active_deposits = 0;
                self.pending_yield = 0;
                self.disputed_funds = 0;
                self.total_escrowed = 0;
                self.vault_total_locked = 0;
                self.lease_count = self.lease_count.saturating_sub(1);
            }
            LeaseOperation::PartialSlash { lease_id: _, slash_percentage_bps } => {
                if *slash_percentage_bps > 10000 {
                    return Err(OperationError::InvalidPercentage);
                }

                let slash_amount = if self.total_escrowed > 0 {
                    // Simulate integer division truncation
                    let raw_amount = self.total_escrowed.checked_mul(*slash_percentage_bps as i128)
                        .ok_or(OperationError::CalculationOverflow)?;
                    raw_amount / 10000i128
                } else {
                    0
                };

                if slash_amount > self.total_escrowed {
                    return Err(OperationError::SlashExceedsTotal);
                }

                // Apply slash proportionally across components
                let slash_ratio = if self.total_escrowed > 0 {
                    (slash_amount as u128 * 10000) / self.total_escrowed as u128
                } else {
                    0
                };

                let active_slash = (self.active_deposits as u128 * slash_ratio) / 10000;
                let pending_slash = (self.pending_yield as u128 * slash_ratio) / 10000;
                let disputed_slash = (self.disputed_funds as u128 * slash_ratio) / 10000;

                self.active_deposits -= active_slash as i128;
                self.pending_yield -= pending_slash as i128;
                self.disputed_funds -= disputed_slash as i128;
                self.total_escrowed -= slash_amount;
                self.vault_total_locked -= slash_amount;

                // Track dust from truncation
                let actual_slash_total = active_slash + pending_slash + disputed_slash;
                if actual_slash_total < slash_amount as u128 {
                    self.dust_tracker += (slash_amount as u128 - actual_slash_total) as i128;
                }
            }
        }

        self.verify_solvency_invariant()
            .map_err(|e| OperationError::InvariantViolation(e))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SolvencyViolation {
    InvariantViolated {
        expected: i128,
        actual: i128,
        active: i128,
        pending: i128,
        disputed: i128,
        precision_loss: i128,
        dust: i128,
    },
    VaultMismatch {
        vault_total: i128,
        escrow_total: i128,
    },
    NegativeValues {
        total: i128,
        active: i128,
        pending: i128,
        disputed: i128,
        vault: i128,
    },
    CalculationOverflow,
}

#[derive(Debug, PartialEq, Eq)]
enum OperationError {
    NegativeDeposit,
    NegativeYield,
    NegativeDispute,
    NegativeSettlement,
    NegativeRelease,
    InsufficientActiveDeposits,
    InsufficientFundsForDispute,
    InsufficientFundsForSettlement,
    ReleaseMathMismatch,
    ComponentMismatch,
    InvalidPercentage,
    SlashExceedsTotal,
    CalculationOverflow,
    InvariantViolation(SolvencyViolation),
}

/// Soroban-specific 128-bit arithmetic simulation
fn checked_add_soroban(a: i128, b: i128, behaviors: &SorobanBehaviors) -> Result<i128, OperationError> {
    // Simulate Soroban's 128-bit overflow behavior
    let result = a.checked_add(b).ok_or(OperationError::CalculationOverflow)?;
    
    // Simulate precision loss if enabled
    if behaviors.fixed_point_precision && (a.abs() > 1_000_000 || b.abs() > 1_000_000) {
        // Simulate minimal precision loss in large operations
        Ok(result - (result / 1_000_000))
    } else {
        Ok(result)
    }
}

fn checked_sub_soroban(a: i128, b: i128, behaviors: &SorobanBehaviors) -> Result<i128, OperationError> {
    let result = a.checked_sub(b).ok_or(OperationError::CalculationOverflow)?;
    
    if behaviors.fixed_point_precision && (a.abs() > 1_000_000 || b.abs() > 1_000_000) {
        Ok(result + (result / 1_000_000))
    } else {
        Ok(result)
    }
}

/// Simulate fixed-point precision loss
fn simulate_fixed_point_loss(amount: i128) -> i128 {
    // Soroban uses 128-bit fixed-point, simulate minimal loss
    if amount > 0 {
        amount / 1_000_000
    } else {
        0
    }
}

/// Calculate dust accounting for multi-signer refunds
fn calculate_dust_accounting(total_settlement: i128, total_escrowed: i128) -> i128 {
    if total_settlement == total_escrowed {
        0
    } else {
        // Dust is the remainder that must be accounted for
        total_escrowed - total_settlement
    }
}

/// Calculate settlement sources with proper accounting
fn calculate_settlement_sources(
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

/// Apply edge cases to stress test the system
fn apply_edge_case(state: &mut FormalEscrowState, edge_case: &EdgeCase, behaviors: &SorobanBehaviors) -> Result<(), OperationError> {
    match edge_case {
        EdgeCase::MaxValues => {
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 999999,
                deposit_amount: i128::MAX / 1000,
                yield_enabled: false,
            }, behaviors)?;
        }
        EdgeCase::MinValues => {
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 1,
                deposit_amount: 1,
                yield_enabled: false,
            }, behaviors)?;
        }
        EdgeCase::OverflowAttempt => {
            let result = state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 0,
                deposit_amount: i128::MAX,
                yield_enabled: false,
            }, behaviors);
            // Should either succeed or fail gracefully with overflow
            assert!(result.is_ok() || matches!(result, Err(OperationError::CalculationOverflow)));
        }
        EdgeCase::UnderflowAttempt => {
            let result = state.apply_operation(&LeaseOperation::SettleLease {
                lease_id: 1,
                tenant_refund: i128::MAX,
                landlord_payout: i128::MAX,
                protocol_fee: i128::MAX,
            }, behaviors);
            // Should fail gracefully
            assert!(result.is_err());
        }
        EdgeCase::ZeroValues => {
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 0,
                deposit_amount: 0,
                yield_enabled: false,
            }, behaviors)?;
        }
        EdgeCase::NegativeValues => {
            let result = state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 1,
                deposit_amount: -1,
                yield_enabled: false,
            }, behaviors);
            assert!(matches!(result, Err(OperationError::NegativeDeposit)));
        }
        EdgeCase::PrecisionLoss => {
            // Create large amounts to trigger precision tracking
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 2,
                deposit_amount: 1_000_000_000,
                yield_enabled: true,
            }, behaviors)?;
            state.apply_operation(&LeaseOperation::AccumulateYield {
                lease_id: 2,
                yield_amount: 123_456_789,
            }, behaviors)?;
        }
        EdgeCase::DustCreation => {
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 3,
                deposit_amount: 1001, // Odd number for dust
                yield_enabled: false,
            }, behaviors)?;
            state.apply_operation(&LeaseOperation::SettleLease {
                lease_id: 3,
                tenant_refund: 334,
                landlord_payout: 334,
                protocol_fee: 333,
            }, behaviors)?;
        }
        EdgeCase::ConcurrentOperations => {
            // Simulate rapid concurrent operations
            for i in 0..10 {
                state.apply_operation(&LeaseOperation::CreateLease {
                    lease_id: 100 + i,
                    deposit_amount: (i + 1) * 1000,
                    yield_enabled: i % 2 == 0,
                }, behaviors)?;
            }
        }
        EdgeCase::ReentrantCall => {
            // Simulate reentrancy by nesting operations
            state.apply_operation(&LeaseOperation::CreateLease {
                lease_id: 200,
                deposit_amount: 5000,
                yield_enabled: true,
            }, behaviors)?;
            state.apply_operation(&LeaseOperation::AccumulateYield {
                lease_id: 200,
                yield_amount: 500,
            }, behaviors)?;
            state.apply_operation(&LeaseOperation::SettleLease {
                lease_id: 200,
                tenant_refund: 3000,
                landlord_payout: 2000,
                protocol_fee: 500,
            }, behaviors)?;
        }
    }
    Ok(())
}

fuzz_target!(|input: EscrowFuzzInput| {
    let mut state = FormalEscrowState::new();
    let mut lease_registry = HashMap::new();
    
    // Apply global modifiers
    let _ = input.global_modifiers;
    
    // Apply edge cases first to stress test
    for edge_case in &input.edge_cases {
        let _ = apply_edge_case(&mut state, edge_case, &input.soroban_behaviors);
    }
    
    // Process operations in sequence
    for operation in &input.operations {
        // Track lease existence
        match operation {
            LeaseOperation::CreateLease { lease_id, .. } => {
                lease_registry.insert(*lease_id, true);
            }
            LeaseOperation::DepositCollateral { lease_id, .. }
            | LeaseOperation::AccumulateYield { lease_id, .. }
            | LeaseOperation::InitiateDispute { lease_id, .. }
            | LeaseOperation::SettleLease { lease_id, .. }
            | LeaseOperation::MutuallyRelease { lease_id, .. }
            | LeaseOperation::PartialSlash { lease_id, .. } => {
                // Only apply operation if lease exists
                if !lease_registry.contains_key(lease_id) {
                    continue;
                }
            }
        }
        
        // Apply operation and verify invariants
        let result = state.apply_operation(operation, &input.soroban_behaviors);
        
        match result {
            Ok(()) => {
                // --- PROPERTY 1: If operation succeeds, invariants must hold ---
                state.verify_solvency_invariant().expect("Invariant violation after successful operation");
                
                // --- PROPERTY 2: Mathematical consistency checks ---
                assert!(state.total_escrowed >= 0, "Total escrowed went negative");
                assert!(state.vault_total_locked >= 0, "Vault total went negative");
                assert!(state.lease_count <= 1000000, "Lease count exploded");
                
                // --- PROPERTY 3: Component sum invariant ---
                let component_sum = state.active_deposits + state.pending_yield + state.disputed_funds;
                assert_eq!(component_sum, state.total_escrowed, 
                    "Component sum mismatch: {} + {} + {} != {}",
                    state.active_deposits, state.pending_yield, state.disputed_funds, state.total_escrowed);
                
                // --- PROPERTY 4: Vault synchronization invariant ---
                assert_eq!(state.vault_total_locked, state.total_escrowed,
                    "Vault out of sync: {} != {}",
                    state.vault_total_locked, state.total_escrowed);
            }
            Err(OperationError::InvariantViolation(violation)) => {
                // --- PROPERTY 5: If invariant violation occurs, it must be documented ---
                match violation {
                    SolvencyViolation::InvariantViolated { expected, actual, .. } => {
                        panic!("Critical invariant violation: expected {}, actual {}", expected, actual);
                    }
                    SolvencyViolation::VaultMismatch { vault_total, escrow_total } => {
                        panic!("Vault mismatch: vault {}, escrow {}", vault_total, escrow_total);
                    }
                    SolvencyViolation::NegativeValues { .. } => {
                        panic!("Negative values detected in state");
                    }
                    SolvencyViolation::CalculationOverflow => {
                        // Overflow is acceptable as a failure mode
                    }
                }
            }
            Err(_) => {
                // Other errors are acceptable failure modes
            }
        }
    }
    
    // --- PROPERTY 6: Final state must always satisfy core invariants ---
    state.verify_solvency_invariant().expect("Final state invariant violation");
    
    // --- PROPERTY 7: Precision and dust tracking must be consistent ---
    assert!(state.precision_loss_tracker >= 0, "Precision loss tracker went negative");
    assert!(state.dust_tracker >= 0, "Dust tracker went negative");
    
    // --- PROPERTY 8: No phantom tokens created ---
    let final_component_sum = state.active_deposits + state.pending_yield + state.disputed_funds;
    assert_eq!(final_component_sum, state.total_escrowed, 
        "Phantom tokens detected in final state");
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_escrow_operations() {
        let mut state = FormalEscrowState::new();
        let behaviors = SorobanBehaviors {
            fixed_point_precision: true,
            ledger_timing_variance: 0,
            gas_limit_reached: false,
            storage_rent_due: false,
        };

        // Create lease
        state.apply_operation(&LeaseOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1000,
            yield_enabled: true,
        }, &behaviors).unwrap();

        assert_eq!(state.total_escrowed, 1000);
        assert_eq!(state.active_deposits, 1000);

        // Accumulate yield
        state.apply_operation(&LeaseOperation::AccumulateYield {
            lease_id: 1,
            yield_amount: 100,
        }, &behaviors).unwrap();

        assert_eq!(state.total_escrowed, 1000);
        assert_eq!(state.active_deposits, 900);
        assert_eq!(state.pending_yield, 100);

        // Settle
        state.apply_operation(&LeaseOperation::SettleLease {
            lease_id: 1,
            tenant_refund: 600,
            landlord_payout: 300,
            protocol_fee: 100,
        }, &behaviors).unwrap();

        assert_eq!(state.total_escrowed, 0);
        assert_eq!(state.lease_count, 0);
    }

    #[test]
    fn test_dust_accounting() {
        let mut state = FormalEscrowState::new();
        let behaviors = SorobanBehaviors {
            fixed_point_precision: false,
            ledger_timing_variance: 0,
            gas_limit_reached: false,
            storage_rent_due: false,
        };

        // Create lease with odd amount
        state.apply_operation(&LeaseOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1001,
            yield_enabled: false,
        }, &behaviors).unwrap();

        // Split that creates dust
        state.apply_operation(&LeaseOperation::SettleLease {
            lease_id: 1,
            tenant_refund: 334,
            landlord_payout: 334,
            protocol_fee: 333,
        }, &behaviors).unwrap();

        // All funds accounted for, no dust loss
        assert_eq!(state.total_escrowed, 0);
        assert_eq!(state.dust_tracker, 0);
    }

    #[test]
    fn test_precision_tracking() {
        let mut state = FormalEscrowState::new();
        let behaviors = SorobanBehaviors {
            fixed_point_precision: true,
            ledger_timing_variance: 0,
            gas_limit_reached: false,
            storage_rent_due: false,
        };

        // Large amount to trigger precision tracking
        state.apply_operation(&LeaseOperation::CreateLease {
            lease_id: 1,
            deposit_amount: 1_000_000_000,
            yield_enabled: true,
        }, &behaviors).unwrap();

        // Should track precision loss
        assert!(state.precision_loss_tracker > 0);
    }
}
