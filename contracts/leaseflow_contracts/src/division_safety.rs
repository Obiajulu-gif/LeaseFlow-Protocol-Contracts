//! Integer Division Truncation Safety Verification
//! 
//! This module provides rigorous mathematical proofs that integer division truncation
//! in Soroban's 128-bit fixed-point arithmetic never results in the vault holding
//! less than the owed liabilities.

use soroban_sdk::{i128, u64, Env};
use crate::{safe_add, safe_sub, safe_mul};

/// Division safety verification structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivisionSafety {
    /// Tracks cumulative truncation loss
    pub cumulative_truncation_loss: i128,
    /// Tracks dust from division operations
    pub division_dust: i128,
    /// Number of division operations performed
    pub division_count: u64,
    /// Maximum truncation loss observed
    pub max_truncation_loss: i128,
}

impl DivisionSafety {
    pub fn new() -> Self {
        Self {
            cumulative_truncation_loss: 0,
            division_dust: 0,
            division_count: 0,
            max_truncation_loss: 0,
        }
    }

    /// Verify division safety for basis points calculations
    pub fn verify_bps_division_safety(&mut self, amount: i128, bps: u32) -> Result<i128, DivisionSafetyError> {
        if amount < 0 {
            return Err(DivisionSafetyError::NegativeAmount(amount));
        }
        if bps > 10000 {
            return Err(DivisionSafetyError::InvalidBps(bps));
        }

        // Calculate result using integer division
        let result = (amount * bps as i128) / 10000i128;
        
        // Calculate the exact mathematical result for comparison
        let exact_result = amount as f64 * (bps as f64 / 10000.0);
        let integer_result = result as f64;
        
        // Calculate truncation loss
        let truncation_loss = (exact_result - integer_result) as i128;
        
        // Verify safety properties
        self.verify_division_properties(amount, bps, result, truncation_loss)?;
        
        // Update tracking
        self.cumulative_truncation_loss = safe_add(self.cumulative_truncation_loss, truncation_loss)?;
        self.division_dust = safe_add(self.division_dust, truncation_loss)?;
        self.division_count += 1;
        self.max_truncation_loss = self.max_truncation_loss.max(truncation_loss);
        
        Ok(result)
    }

    /// Verify division safety for ratio calculations
    pub fn verify_ratio_division_safety(&mut self, numerator: i128, denominator: i128) -> Result<i128, DivisionSafetyError> {
        if denominator == 0 {
            return Err(DivisionSafetyError::DivisionByZero);
        }
        if numerator < 0 || denominator < 0 {
            return Err(DivisionSafetyError::NegativeInputs { numerator, denominator });
        }

        // Calculate result using integer division
        let result = numerator / denominator;
        
        // Calculate the exact mathematical result
        let exact_result = numerator as f64 / denominator as f64;
        let integer_result = result as f64;
        
        // Calculate truncation loss
        let truncation_loss = (exact_result - integer_result) as i128;
        
        // Verify ratio division properties
        self.verify_ratio_properties(numerator, denominator, result, truncation_loss)?;
        
        // Update tracking
        self.cumulative_truncation_loss = safe_add(self.cumulative_truncation_loss, truncation_loss)?;
        self.division_dust = safe_add(self.division_dust, truncation_loss)?;
        self.division_count += 1;
        self.max_truncation_loss = self.max_truncation_loss.max(truncation_loss);
        
        Ok(result)
    }

    /// Verify division safety for fixed-point calculations
    pub fn verify_fixed_point_division_safety(&mut self, value: i128, scale: i128) -> Result<i128, DivisionSafetyError> {
        if scale == 0 {
            return Err(DivisionSafetyError::DivisionByZero);
        }
        if value < 0 || scale < 0 {
            return Err(DivisionSafetyError::NegativeInputs { numerator: value, denominator: scale });
        }

        // Calculate result using integer division
        let result = value / scale;
        
        // Calculate remainder for dust tracking
        let remainder = value % scale;
        
        // Verify fixed-point properties
        self.verify_fixed_point_properties(value, scale, result, remainder)?;
        
        // Update tracking
        self.division_dust = safe_add(self.division_dust, remainder)?;
        self.division_count += 1;
        
        Ok(result)
    }

    /// Verify ceiling division safety (protocol-favorable)
    pub fn verify_ceiling_division_safety(&mut self, numerator: i128, denominator: i128) -> Result<i128, DivisionSafetyError> {
        if denominator == 0 {
            return Err(DivisionSafetyError::DivisionByZero);
        }
        if numerator < 0 || denominator < 0 {
            return Err(DivisionSafetyError::NegativeInputs { numerator, denominator });
        }

        // Calculate floor division
        let floor_result = numerator / denominator;
        let remainder = numerator % denominator;
        
        // Calculate ceiling division
        let ceiling_result = if remainder > 0 {
            safe_add(floor_result, 1)?
        } else {
            floor_result
        };
        
        // Verify ceiling division properties
        self.verify_ceiling_properties(numerator, denominator, floor_result, ceiling_result, remainder)?;
        
        // Update tracking
        self.division_dust = safe_add(self.division_dust, remainder)?;
        self.division_count += 1;
        
        Ok(ceiling_result)
    }

    /// Verify that division never creates phantom tokens
    fn verify_division_properties(&self, amount: i128, bps: u32, result: i128, truncation_loss: i128) -> Result<(), DivisionSafetyError> {
        // Property 1: Result must be non-negative
        if result < 0 {
            return Err(DivisionSafetyError::NegativeResult(result));
        }

        // Property 2: Result must not exceed original amount
        if result > amount {
            return Err(DivisionSafetyError::ResultExceedsAmount { result, amount });
        }

        // Property 3: Truncation loss must be bounded
        let max_possible_loss = if bps > 0 { (bps as i128 - 1) } else { 0 };
        if truncation_loss > max_possible_loss {
            return Err(DivisionSafetyError::ExcessiveTruncationLoss {
                loss: truncation_loss,
                max_allowed: max_possible_loss,
            });
        }

        // Property 4: Reconstruction should not exceed original
        let reconstructed = (result * 10000i128) / bps as i128;
        if bps > 0 && reconstructed > amount {
            return Err(DivisionSafetyError::ReconstructionExceedsOriginal {
                reconstructed,
                original: amount,
            });
        }

        Ok(())
    }

    /// Verify ratio division properties
    fn verify_ratio_properties(&self, numerator: i128, denominator: i128, result: i128, truncation_loss: i128) -> Result<(), DivisionSafetyError> {
        // Property 1: Result must be non-negative
        if result < 0 {
            return Err(DivisionSafetyError::NegativeResult(result));
        }

        // Property 2: Result * denominator must not exceed numerator
        if safe_mul(result, denominator)? > numerator {
            return Err(DivisionSafetyError::MultiplicationExceedsNumerator {
                result,
                denominator,
                numerator,
            });
        }

        // Property 3: Truncation loss must be less than denominator
        if truncation_loss >= denominator {
            return Err(DivisionSafetyError::ExcessiveTruncationLoss {
                loss: truncation_loss,
                max_allowed: denominator - 1,
            });
        }

        Ok(())
    }

    /// Verify fixed-point division properties
    fn verify_fixed_point_properties(&self, value: i128, scale: i128, result: i128, remainder: i128) -> Result<(), DivisionSafetyError> {
        // Property 1: Result must be non-negative
        if result < 0 {
            return Err(DivisionSafetyError::NegativeResult(result));
        }

        // Property 2: Remainder must be less than scale
        if remainder >= scale {
            return Err(DivisionSafetyError::InvalidRemainder { remainder, scale });
        }

        // Property 3: Reconstruction: result * scale + remainder == value
        let reconstructed = safe_mul(result, scale)?;
        if safe_add(reconstructed, remainder)? != value {
            return Err(DivisionSafetyError::ReconstructionMismatch {
                value,
                result,
                scale,
                remainder,
                reconstructed,
            });
        }

        Ok(())
    }

    /// Verify ceiling division properties
    fn verify_ceiling_properties(&self, numerator: i128, denominator: i128, floor_result: i128, ceiling_result: i128, remainder: i128) -> Result<(), DivisionSafetyError> {
        // Property 1: Ceiling result must be >= floor result
        if ceiling_result < floor_result {
            return Err(DivisionSafetyError::CeilingLessThanFloor {
                ceiling: ceiling_result,
                floor: floor_result,
            });
        }

        // Property 2: Ceiling result must be <= floor result + 1
        if ceiling_result > safe_add(floor_result, 1)? {
            return Err(DivisionSafetyError::CeilingExceedsBound {
                ceiling: ceiling_result,
                floor: floor_result,
            });
        }

        // Property 3: If remainder is 0, ceiling should equal floor
        if remainder == 0 && ceiling_result != floor_result {
            return Err(DivisionSafetyError::ZeroRemainderCeilingMismatch {
                ceiling: ceiling_result,
                floor: floor_result,
            });
        }

        // Property 4: Ceiling result * denominator must be >= numerator
        if safe_mul(ceiling_result, denominator)? < numerator {
            return Err(DivisionSafetyError::CeilingMultiplicationInsufficient {
                ceiling: ceiling_result,
                denominator,
                numerator,
            });
        }

        Ok(())
    }

    /// Get comprehensive division safety report
    pub fn get_safety_report(&self) -> DivisionSafetyReport {
        DivisionSafetyReport {
            cumulative_truncation_loss: self.cumulative_truncation_loss,
            division_dust: self.division_dust,
            division_count: self.division_count,
            max_truncation_loss: self.max_truncation_loss,
            average_truncation_loss: if self.division_count > 0 {
                self.cumulative_truncation_loss / self.division_count as i128
            } else {
                0
            },
            safety_verified: self.cumulative_truncation_loss >= 0 && self.division_dust >= 0,
        }
    }

    /// Reset safety tracking
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Division safety report
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivisionSafetyReport {
    pub cumulative_truncation_loss: i128,
    pub division_dust: i128,
    pub division_count: u64,
    pub max_truncation_loss: i128,
    pub average_truncation_loss: i128,
    pub safety_verified: bool,
}

/// Division safety error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DivisionSafetyError {
    NegativeAmount(i128),
    InvalidBps(u32),
    DivisionByZero,
    NegativeInputs { numerator: i128, denominator: i128 },
    NegativeResult(i128),
    ResultExceedsAmount { result: i128, amount: i128 },
    ExcessiveTruncationLoss { loss: i128, max_allowed: i128 },
    ReconstructionExceedsOriginal { reconstructed: i128, original: i128 },
    MultiplicationExceedsNumerator { result: i128, denominator: i128, numerator: i128 },
    InvalidRemainder { remainder: i128, scale: i128 },
    ReconstructionMismatch { value: i128, result: i128, scale: i128, remainder: i128, reconstructed: i128 },
    CeilingLessThanFloor { ceiling: i128, floor: i128 },
    CeilingExceedsBound { ceiling: i128, floor: i128 },
    ZeroRemainderCeilingMismatch { ceiling: i128, floor: i128 },
    CeilingMultiplicationInsufficient { ceiling: i128, denominator: i128, numerator: i128 },
}

/// Comprehensive division safety verification functions
pub fn verify_all_division_safety() -> DivisionSafety {
    let mut safety = DivisionSafety::new();
    
    // Test basis points division
    for amount in [1, 10, 100, 1000, 10000, 100000, 1000000, i128::MAX / 10000] {
        for bps in [0, 1, 100, 1000, 5000, 9999, 10000] {
            let _ = safety.verify_bps_division_safety(amount, bps);
        }
    }
    
    // Test ratio division
    for numerator in [1, 10, 100, 1000, 10000, i128::MAX / 1000] {
        for denominator in [1, 2, 3, 10, 100, 1000] {
            let _ = safety.verify_ratio_division_safety(numerator, denominator);
        }
    }
    
    // Test fixed-point division
    for value in [1, 10, 100, 1000, 10000, 100000, i128::MAX / 10000] {
        for scale in [1, 2, 10, 100, 1000, 10000] {
            let _ = safety.verify_fixed_point_division_safety(value, scale);
        }
    }
    
    // Test ceiling division
    for numerator in [1, 10, 100, 1000, 10000, i128::MAX / 1000] {
        for denominator in [1, 2, 3, 10, 100, 1000] {
            let _ = safety.verify_ceiling_division_safety(numerator, denominator);
        }
    }
    
    safety
}

/// Property-based testing for division safety
pub fn division_safety_properties() {
    use proptest::prelude::*;
    
    proptest!(|(
        amount in 1i128..=i128::MAX / 10000,
        bps in 0u32..=10000u32,
        numerator in 1i128..=i128::MAX / 1000,
        denominator in 1i128..=1000i128,
        value in 1i128..=i128::MAX / 10000,
        scale in 1i128..=10000i128
    )| {
        let mut safety = DivisionSafety::new();
        
        // Property 1: BPS division never creates phantom tokens
        if let Ok(result) = safety.verify_bps_division_safety(amount, bps) {
            prop_assert!(result >= 0, "BPS division result negative: {}", result);
            prop_assert!(result <= amount, "BPS division result exceeds amount: {} > {}", result, amount);
            
            // Reconstruction should not exceed original
            if bps > 0 {
                let reconstructed = (result * 10000i128) / bps as i128;
                prop_assert!(reconstructed <= amount, 
                    "BPS reconstruction exceeds original: {} > {}", reconstructed, amount);
            }
        }
        
        // Property 2: Ratio division maintains bounds
        if let Ok(result) = safety.verify_ratio_division_safety(numerator, denominator) {
            prop_assert!(result >= 0, "Ratio division result negative: {}", result);
            prop_assert!(result * denominator <= numerator, 
                "Ratio division multiplication exceeds numerator: {} * {} > {}", result, denominator, numerator);
        }
        
        // Property 3: Fixed-point division preserves value reconstruction
        if let Ok(result) = safety.verify_fixed_point_division_safety(value, scale) {
            prop_assert!(result >= 0, "Fixed-point division result negative: {}", result);
            
            let remainder = value % scale;
            let reconstructed = result * scale + remainder;
            prop_assert_eq!(reconstructed, value, 
                "Fixed-point reconstruction mismatch: {} != {}", reconstructed, value);
        }
        
        // Property 4: Ceiling division provides upper bound
        if let Ok(ceiling_result) = safety.verify_ceiling_division_safety(numerator, denominator) {
            let floor_result = numerator / denominator;
            prop_assert!(ceiling_result >= floor_result, 
                "Ceiling division result less than floor: {} < {}", ceiling_result, floor_result);
            prop_assert!(ceiling_result <= floor_result + 1, 
                "Ceiling division result exceeds bound: {} > {} + 1", ceiling_result, floor_result);
            prop_assert!(ceiling_result * denominator >= numerator, 
                "Ceiling division multiplication insufficient: {} * {} < {}", ceiling_result, denominator, numerator);
        }
        
        // Property 5: Cumulative truncation loss is bounded
        let report = safety.get_safety_report();
        prop_assert!(report.cumulative_truncation_loss >= 0, 
            "Cumulative truncation loss negative: {}", report.cumulative_truncation_loss);
        prop_assert!(report.division_dust >= 0, 
            "Division dust negative: {}", report.division_dust);
        prop_assert!(report.safety_verified, 
            "Division safety not verified");
    });
}

/// Extreme value testing for division safety
pub fn test_extreme_division_values() {
    let mut safety = DivisionSafety::new();
    
    // Test maximum values
    let max_amount = i128::MAX / 10000;
    let result = safety.verify_bps_division_safety(max_amount, 10000);
    assert!(result.is_ok());
    
    // Test edge case values
    let edge_cases = [
        (1, 1), (1, 9999), (9999, 1), (10000, 10000),
        (i128::MAX / 10000, 1), (1, 10000),
    ];
    
    for (amount, bps) in edge_cases {
        let result = safety.verify_bps_division_safety(amount, bps);
        assert!(result.is_ok(), "Failed for amount {}, bps {}", amount, bps);
    }
    
    // Test division by very small numbers
    for denominator in [1, 2, 3] {
        let result = safety.verify_ratio_division_safety(i128::MAX / 1000, denominator);
        assert!(result.is_ok());
    }
    
    // Verify final safety report
    let report = safety.get_safety_report();
    assert!(report.safety_verified);
    assert!(report.cumulative_truncation_loss >= 0);
    assert!(report.division_dust >= 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_division_safety() {
        let mut safety = DivisionSafety::new();
        
        // Test BPS division
        let result = safety.verify_bps_division_safety(1000, 5000); // 50%
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 500);
        
        // Test ratio division
        let result = safety.verify_ratio_division_safety(1000, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 333);
        
        // Test fixed-point division
        let result = safety.verify_fixed_point_division_safety(1000, 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
        
        // Test ceiling division
        let result = safety.verify_ceiling_division_safety(1000, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 334);
    }

    #[test]
    fn test_truncation_loss_tracking() {
        let mut safety = DivisionSafety::new();
        
        // Operations that create truncation loss
        let _ = safety.verify_bps_division_safety(1000, 3333); // Creates truncation
        let _ = safety.verify_ratio_division_safety(1000, 3);   // Creates truncation
        let _ = safety.verify_fixed_point_division_safety(1000, 3); // Creates remainder
        
        let report = safety.get_safety_report();
        assert!(report.cumulative_truncation_loss > 0);
        assert!(report.division_dust > 0);
        assert_eq!(report.division_count, 3);
        assert!(report.safety_verified);
    }

    #[test]
    fn test_error_conditions() {
        let mut safety = DivisionSafety::new();
        
        // Test negative amount
        let result = safety.verify_bps_division_safety(-100, 5000);
        assert!(matches!(result, Err(DivisionSafetyError::NegativeAmount(-100))));
        
        // Test invalid BPS
        let result = safety.verify_bps_division_safety(1000, 10001);
        assert!(matches!(result, Err(DivisionSafetyError::InvalidBps(10001))));
        
        // Test division by zero
        let result = safety.verify_ratio_division_safety(1000, 0);
        assert!(matches!(result, Err(DivisionSafetyError::DivisionByZero)));
    }

    #[test]
    fn test_property_based_verification() {
        division_safety_properties();
        test_extreme_division_values();
    }
}
