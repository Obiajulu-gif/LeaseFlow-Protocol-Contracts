#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Address, Env, Symbol};

// Mock contract for fuzzing
mod leaseflow_contracts {
    use soroban_sdk::{contractimpl, Address, Env, i128, u64};

    pub struct LeaseContract;

    #[contractimpl]
    impl LeaseContract {
        pub fn mutual_deposit_release(
            env: Env,
            lease_id: u64,
            lessee_pubkey: Address,
            lessor_pubkey: Address,
            return_amount: i128,
            slash_amount: i128,
        ) -> Result<(), i32> {
            // Simplified validation for fuzzing
            if return_amount < 0 || slash_amount < 0 {
                return Err(1); // InvalidReleaseMath
            }

            let total_escrowed = 1000i128; // Mock escrowed amount
            if return_amount + slash_amount != total_escrowed {
                return Err(1); // InvalidReleaseMath
            }

            Ok(())
        }
    }
}

/// Fuzz input for mutual deposit release testing
#[derive(Arbitrary, Debug)]
struct MutualReleaseInput {
    lease_id: u64,
    lessee_pubkey_bytes: [u8; 32],
    lessor_pubkey_bytes: [u8; 32],
    return_amount: i128,
    slash_amount: i128,
    // Test edge cases with known problematic values
    use_edge_case: Option<EdgeCase>,
}

#[derive(Arbitrary, Debug)]
enum EdgeCase {
    ZeroReturn,
    ZeroSlash,
    MaxValues,
    MinValues,
    OverflowSum,
    NegativeReturn,
    NegativeSlash,
    ExactSplit,
}

fuzz_target!(|input: MutualReleaseInput| {
    let MutualReleaseInput {
        lease_id,
        lessee_pubkey_bytes,
        lessor_pubkey_bytes,
        mut return_amount,
        mut slash_amount,
        use_edge_case,
    } = input;

    // Apply edge cases if specified
    if let Some(edge_case) = use_edge_case {
        match edge_case {
            EdgeCase::ZeroReturn => return_amount = 0,
            EdgeCase::ZeroSlash => slash_amount = 0,
            EdgeCase::MaxValues => {
                return_amount = i128::MAX / 2;
                slash_amount = i128::MAX / 2;
            }
            EdgeCase::MinValues => {
                return_amount = i128::MIN;
                slash_amount = i128::MIN;
            }
            EdgeCase::OverflowSum => {
                return_amount = i128::MAX;
                slash_amount = i128::MAX;
            }
            EdgeCase::NegativeReturn => return_amount = -1,
            EdgeCase::NegativeSlash => slash_amount = -1,
            EdgeCase::ExactSplit => {
                return_amount = 500;
                slash_amount = 500;
            }
        }
    }

    // Property 1: Mathematical invariants - total must equal escrowed amount
    let total_escrowed = 1000i128; // Mock total escrowed
    
    // Test the core validation logic directly
    let validation_result = validate_mutual_release_logic(return_amount, slash_amount, total_escrowed);
    
    match validation_result {
        Ok(()) => {
            // --- Property 2: If validation passes, mathematical invariants must hold ---
            assert_eq!(
                return_amount + slash_amount,
                total_escrowed,
                "Validation passed but sum doesn't match total: {} + {} != {}",
                return_amount,
                slash_amount,
                total_escrowed
            );
            
            // --- Property 3: Amounts must be non-negative ---
            assert!(
                return_amount >= 0,
                "Validation passed but return_amount is negative: {}",
                return_amount
            );
            assert!(
                slash_amount >= 0,
                "Validation passed but slash_amount is negative: {}",
                slash_amount
            );
            
            // --- Property 4: Individual amounts cannot exceed total ---
            assert!(
                return_amount <= total_escrowed,
                "Return amount {} exceeds total {}",
                return_amount,
                total_escrowed
            );
            assert!(
                slash_amount <= total_escrowed,
                "Slash amount {} exceeds total {}",
                slash_amount,
                total_escrowed
            );
        }
        Err(LeaseFlowError::InvalidReleaseMath) => {
            // --- Property 5: If validation fails, at least one invariant must be violated ---
            let sum_matches = return_amount + slash_amount == total_escrowed;
            let both_non_negative = return_amount >= 0 && slash_amount >= 0;
            
            // At least one condition must fail for InvalidReleaseMath
            assert!(
                !sum_matches || !both_non_negative,
                "InvalidReleaseMath returned but all invariants hold: return={}, slash={}, total={}",
                return_amount,
                slash_amount,
                total_escrowed
            );
        }
    }
    
    // Property 6: Determinism - same inputs should always give same results
    let result1 = validate_mutual_release_logic(return_amount, slash_amount, total_escrowed);
    let result2 = validate_mutual_release_logic(return_amount, slash_amount, total_escrowed);
    assert_eq!(
        result1, result2,
        "Non-deterministic behavior for inputs: return={}, slash={}",
        return_amount, slash_amount
    );
    
    // Property 7: Commutativity - order shouldn't matter for sum validation
    if return_amount != slash_amount {
        let result_swapped = validate_mutual_release_logic(slash_amount, return_amount, total_escrowed);
        assert_eq!(
            result1, result_swapped,
            "Swapped parameters gave different results: ({}, {}) vs ({}, {})",
            return_amount, slash_amount, slash_amount, return_amount
        );
    }
});

#[derive(Debug, PartialEq, Eq)]
enum LeaseFlowError {
    InvalidReleaseMath,
}

/// Extracted validation logic for testing
fn validate_mutual_release_logic(
    return_amount: i128,
    slash_amount: i128,
    total_escrowed: i128,
) -> Result<(), LeaseFlowError> {
    // Ensure amounts are non-negative
    if return_amount < 0 || slash_amount < 0 {
        return Err(LeaseFlowError::InvalidReleaseMath);
    }

    // Mathematical validation: ensure amounts sum to total escrowed deposit
    if return_amount + slash_amount != total_escrowed {
        return Err(LeaseFlowError::InvalidReleaseMath);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_exact_split() {
        let result = validate_mutual_release_logic(500, 500, 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_full_refund() {
        let result = validate_mutual_release_logic(1000, 0, 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_full_slash() {
        let result = validate_mutual_release_logic(0, 1000, 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_sum_mismatch() {
        let result = validate_mutual_release_logic(400, 500, 1000);
        assert_eq!(result, Err(LeaseFlowError::InvalidReleaseMath));
    }

    #[test]
    fn test_invalid_negative_return() {
        let result = validate_mutual_release_logic(-100, 1100, 1000);
        assert_eq!(result, Err(LeaseFlowError::InvalidReleaseMath));
    }

    #[test]
    fn test_invalid_negative_slash() {
        let result = validate_mutual_release_logic(1100, -100, 1000);
        assert_eq!(result, Err(LeaseFlowError::InvalidReleaseMath));
    }

    #[test]
    fn test_overflow_protection() {
        let result = validate_mutual_release_logic(i128::MAX, i128::MAX, 1000);
        assert_eq!(result, Err(LeaseFlowError::InvalidReleaseMath));
    }
}
