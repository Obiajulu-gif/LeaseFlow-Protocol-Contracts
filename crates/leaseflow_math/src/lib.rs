#![no_std]

/// Calculates the total rental cost given a duration (seconds) and a rate
/// (cost per second). Returns `None` on overflow.
pub fn calculate_total_cost(duration_secs: u64, rate_per_sec: u64) -> Option<u64> {
    duration_secs.checked_mul(rate_per_sec)
}

/// Safely splits a deposit between landlord and tenant based on basis points (0-10000).
/// Basis points: 10000 = 100%.
/// This ensures no tokens are stuck due to rounding by calculating the landlord's
/// share and giving the remainder to the tenant.
pub fn calculate_deposit_split(total_deposit: i128, landlord_bps: u32) -> Option<(i128, i128)> {
    let landlord_pct = (landlord_bps.min(10000)) as i128;
    
    // Intermediate calculation to prevent overflow before division
    let landlord_share = total_deposit.checked_mul(landlord_pct)? / 10000;
    let tenant_share = total_deposit.checked_sub(landlord_share)?;

    Some((landlord_share, tenant_share))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// This test proves "Property 1: Conservation of Balance"
        /// It simulates thousands of different deposit amounts and split ratios.
        /// It verifies that landlord_share + tenant_share is ALWAYS exactly total_deposit.
        #[test]
        fn test_deposit_split_always_equals_total(
            total in 0..i128::MAX / 10000,
            bps in 0..10000u32
        ) {
            let result = calculate_deposit_split(total, bps);
            assert!(result.is_some());
            let (landlord, tenant) = result.unwrap();
            
            // Prove Conservation: The sum MUST exactly match the input
            prop_assert_eq!(
                landlord + tenant,
                total,
                "Internal accounting mismatch: {}+{} != {} (bps={})",
                landlord,
                tenant,
                total,
                bps
            );

            // Prove Non-negativity: No negative shares from non-negative total
            prop_assert!(landlord >= 0);
            prop_assert!(tenant >= 0);

            // Prove Fairness: Landlord share must be proportional
            // With integer math: (total * bps) / 10000
            let expected_landlord = (total * bps as i128) / 10000;
            prop_assert_eq!(landlord, expected_landlord);
        }


        #[test]
        fn test_extreme_amounts_caught_by_checked_math(
            total in i128::MAX-10000..i128::MAX,
            bps in 1..10000u32
        ) {
            // This test verifies that we correctly return None on overflow
            // instead of producing "ghost tokens" or wrapping around.
            let result = calculate_deposit_split(total, bps);
            
            // If total is close to MAX and bps > 0, total*bps should overflow
            if bps > 0 {
                prop_assert!(result.is_none());
            }
        }
    }
}



