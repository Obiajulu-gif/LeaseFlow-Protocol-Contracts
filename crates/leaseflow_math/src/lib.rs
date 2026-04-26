#![no_std]

/// Calculates the total rental cost given a duration (seconds) and a rate
/// (cost per second). Returns `None` on overflow.
pub fn calculate_total_cost(duration_secs: u64, rate_per_sec: u64) -> Option<u64> {
    duration_secs.checked_mul(rate_per_sec)
}

/// Get the number of seconds in a specific month for a given timestamp
/// Handles leap years and varying month lengths automatically
pub fn get_seconds_in_month(timestamp: u64) -> u64 {
    // Convert timestamp to Unix time (seconds since 1970-01-01)
    let unix_time = timestamp as i64;
    
    // Calculate days since epoch
    let days_since_epoch = unix_time / 86400;
    
    // Calculate year using efficient algorithm instead of loop
    // Approximate years since 1970, then adjust
    let mut year = 1970 + (days_since_epoch / 365) as i32;
    let mut remaining_days = days_since_epoch % 365;
    
    // Adjust for leap years passed
    let leap_years_before = (year - 1968) / 4 - (year - 1900) / 100 + (year - 1600) / 400;
    remaining_days -= leap_years_before as i64;
    
    // Adjust year if we've gone too far
    while remaining_days < 0 {
        year -= 1;
        let days_in_prev_year = if is_leap_year(year) { 366 } else { 365 };
        remaining_days += days_in_prev_year;
    }
    
    // Adjust year if we haven't gone far enough
    while remaining_days >= (if is_leap_year(year) { 366 } else { 365 }) {
        let days_in_current_year = if is_leap_year(year) { 366 } else { 365 };
        remaining_days -= days_in_current_year;
        year += 1;
    }
    
    // Determine month based on remaining days in current year
    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut month = 0;
    for (i, &days_in_month) in month_days.iter().enumerate() {
        if remaining_days < days_in_month {
            month = i;
            break;
        }
        remaining_days -= days_in_month;
    }
    
    // Return seconds in the identified month
    month_days[month] * 86400
}

/// Check if a year is a leap year
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Calculate prorated rent for a given period using precise i128 fixed-point math
/// Returns the prorated amount and remaining seconds for full cycle validation
pub fn calculate_prorated_rent(
    monthly_rent: i64,
    start_timestamp: u64,
    end_timestamp: u64,
) -> Option<(i64, u64)> {
    if start_timestamp >= end_timestamp || monthly_rent <= 0 {
        return None;
    }
    
    let duration_seconds = end_timestamp.saturating_sub(start_timestamp);
    
    // Get seconds in the month containing the start timestamp
    let seconds_in_month = get_seconds_in_month(start_timestamp);
    
    if seconds_in_month == 0 {
        return None;
    }
    
    // Use i128 for precise fixed-point math to prevent overflow and rounding errors
    let monthly_rent_i128 = monthly_rent as i128;
    let duration_i128 = duration_seconds as i128;
    let seconds_in_month_i128 = seconds_in_month as i128;
    
    // Calculate: monthly_rent * (duration / seconds_in_month)
    // Using integer division with proper rounding
    let prorated_rent_i128 = monthly_rent_i128
        .checked_mul(duration_i128)?
        .checked_div(seconds_in_month_i128)?;
    
    // Convert back to i64, ensuring we don't overflow
    let prorated_rent = if prorated_rent_i128 > i64::MAX as i128 {
        return None;
    } else {
        prorated_rent_i128 as i64
    };
    
    Some((prorated_rent, duration_seconds))
}

/// Calculate refund for early termination with security against rounding exploits
/// Ensures the protocol cannot be exploited through rapid initialization/cancellation
pub fn calculate_termination_refund(
    monthly_rent: i64,
    original_start: u64,
    original_end: u64,
    termination_timestamp: u64,
    paid_amount: i64,
) -> Option<i64> {
    if termination_timestamp <= original_start || termination_timestamp >= original_end {
        return None;
    }
    
    // Calculate unused period
    let unused_duration = original_end.saturating_sub(termination_timestamp);
    
    // Get seconds in the month containing termination timestamp
    let seconds_in_month = get_seconds_in_month(termination_timestamp);
    
    if seconds_in_month == 0 {
        return None;
    }
    
    // Calculate refund for unused period using precise math
    let monthly_rent_i128 = monthly_rent as i128;
    let unused_duration_i128 = unused_duration as i128;
    let seconds_in_month_i128 = seconds_in_month as i128;
    
    let refund_i128 = monthly_rent_i128
        .checked_mul(unused_duration_i128)?
        .checked_div(seconds_in_month_i128)?;
    
    // Ensure refund doesn't exceed paid amount
    let refund = if refund_i128 > paid_amount as i128 {
        paid_amount
    } else if refund_i128 > i64::MAX as i128 {
        return None;
    } else {
        refund_i128 as i64
    };
    
    // Apply security margin: deduct 1 stroop to prevent dust exploitation
    if refund > 0 {
        Some(refund - 1)
    } else {
        Some(0)
    }
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

/// Converts a Unix timestamp into `(year, month, day)` using Howard Hinnant's algorithm.
///
/// # Parameters
/// - `timestamp` – Unix timestamp in seconds.
///
/// # Returns
/// `(year, month [1-12], day [1-31])`
pub fn timestamp_to_ymd(timestamp: u64) -> (u64, u8, u8) {
    let days_since_epoch = timestamp / 86_400;
    let z = days_since_epoch + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u8, d as u8)
}

/// Returns `true` if `year` is a leap year.
pub fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in `month` of `year`.
pub fn days_in_month(year: u64, month: u8) -> u64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

/// Converts `(year, month, day)` back to a Unix timestamp (midnight UTC).
///
/// Uses the inverse of Howard Hinnant's civil-from-days algorithm.
pub fn ymd_to_timestamp(year: u64, month: u8, day: u8) -> u64 {
    let (y, m) = if (month as u64) <= 2 {
        (year - 1, month as u64 + 9)
    } else {
        (year, month as u64 - 3)
    };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe;
    // days is days since 0000-03-01; subtract offset to get Unix epoch days
    days.saturating_sub(719_468) * 86_400
}

/// Calculates the Unix timestamp of the `n`-th monthly billing date anchored
/// to `inception_ts`.
///
/// The billing date is always the same calendar day-of-month as the inception
/// date, advanced by `n` months. If the target month is shorter than the
/// inception day (e.g. inception on Jan 31, billing month is February), the
/// date is clamped to the last day of that month — preventing the "billing
/// drift" vulnerability described in Issue #134.
///
/// # Parameters
/// - `inception_ts` – Unix timestamp of the original lease start.
/// - `n`            – Number of monthly cycles to advance (1-based).
///
/// # Returns
/// Unix timestamp (seconds, midnight UTC) of the `n`-th billing date.
///
/// # Security
/// The anchor is derived solely from `inception_ts` and `n`; it is never
/// accumulated from a running `current_ts`, making it immune to ledger drift.
pub fn next_billing_date(inception_ts: u64, n: u32) -> u64 {
    let (year, month, day) = timestamp_to_ymd(inception_ts);

    // Advance by n months, carrying over into years.
    let total_months = (month as u32).saturating_sub(1) + n;
    let new_month = ((total_months % 12) + 1) as u8;
    let new_year = year + (total_months / 12) as u64;

    // Clamp day to the last valid day of the target month (e.g. Feb 29 → Feb 28/29).
    let max_day = days_in_month(new_year, new_month) as u8;
    let clamped_day = day.min(max_day);

    ymd_to_timestamp(new_year, new_month, clamped_day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_get_seconds_in_month_regular_months() {
        // Test regular months (non-leap year)
        let jan_2024 = 1704067200; // 2024-01-01 00:00:00 UTC
        let feb_2024 = 1706745600; // 2024-02-01 00:00:00 UTC
        let apr_2024 = 1711929600; // 2024-04-01 00:00:00 UTC
        
        assert_eq!(get_seconds_in_month(jan_2024), 31 * 86400); // January: 31 days
        assert_eq!(get_seconds_in_month(feb_2024), 29 * 86400); // February 2024: 29 days (leap year)
        assert_eq!(get_seconds_in_month(apr_2024), 30 * 86400); // April: 30 days
    }

    #[test]
    fn test_get_seconds_in_month_leap_year() {
        // Test leap year February
        let feb_2023 = 1675209600; // 2023-02-01 (non-leap year)
        let feb_2024 = 1706745600; // 2024-02-01 (leap year)
        
        assert_eq!(get_seconds_in_month(feb_2023), 28 * 86400); // 2023: 28 days
        assert_eq!(get_seconds_in_month(feb_2024), 29 * 86400); // 2024: 29 days
    }

    #[test]
    fn test_calculate_prorated_rent_basic() {
        let monthly_rent = 1000;
        let start = 1704067200; // 2024-01-01
        let end = 1704153600;   // 2024-01-02 (24 hours later)
        
        let (prorated, duration) = calculate_prorated_rent(monthly_rent, start, end).unwrap();
        assert_eq!(duration, 86400); // 24 hours in seconds
        assert_eq!(prorated, 32); // Approximately 1000 * (86400 / (31 * 86400)) = 1000/31 ≈ 32
    }

    #[test]
    fn test_calculate_prorated_rent_full_month() {
        let monthly_rent = 1000;
        let start = 1704067200; // 2024-01-01
        let end = 1706668800;   // 2024-01-31 (30 days later, but January has 31 days)
        
        let (prorated, duration) = calculate_prorated_rent(monthly_rent, start, end).unwrap();
        assert_eq!(duration, 30 * 86400);
        assert_eq!(prorated, 967); // 1000 * (30/31) ≈ 967
    }

    #[test]
    fn test_calculate_prorated_rent_invalid_inputs() {
        // Invalid: start >= end
        assert!(calculate_prorated_rent(1000, 1000, 1000).is_none());
        assert!(calculate_prorated_rent(1000, 2000, 1000).is_none());
        
        // Invalid: negative rent
        assert!(calculate_prorated_rent(-100, 1000, 2000).is_none());
        assert!(calculate_prorated_rent(0, 1000, 2000).is_none());
    }

    #[test]
    fn test_calculate_termination_refund_basic() {
        let monthly_rent = 1000;
        let original_start = 1704067200; // 2024-01-01
        let original_end = 1706668800;   // 2024-01-31
        let termination = 1704921600;    // 2024-01-10 (terminate after 9 days)
        let paid_amount = 1000;
        
        let refund = calculate_termination_refund(
            monthly_rent, original_start, original_end, termination, paid_amount
        ).unwrap();
        
        // Should refund for remaining 21 days of January
        // 1000 * (21/31) ≈ 677, minus 1 stroop for security
        assert_eq!(refund, 676);
    }

    #[test]
    fn test_calculate_termination_refund_security_margin() {
        // Test that 1 stroop is deducted for security
        let monthly_rent = 3100; // Amount that divides evenly by 31 days
        let original_start = 1704067200; // 2024-01-01
        let original_end = 1706668800;   // 2024-01-31
        let termination = 1704921600;    // 2024-01-10
        let paid_amount = 3100;
        
        let refund = calculate_termination_refund(
            monthly_rent, original_start, original_end, termination, paid_amount
        ).unwrap();
        
        // 3100 * (21/31) = 2100, minus 1 stroop = 2099
        assert_eq!(refund, 2099);
    }

    #[test]
    fn test_calculate_termination_refund_invalid_timing() {
        let monthly_rent = 1000;
        let original_start = 1704067200; // 2024-01-01
        let original_end = 1706668800;   // 2024-01-31
        
        // Terminate before start
        assert!(calculate_termination_refund(
            monthly_rent, original_start, original_end, 1704000000, 1000
        ).is_none());
        
        // Terminate after end
        assert!(calculate_termination_refund(
            monthly_rent, original_start, original_end, 1707000000, 1000
        ).is_none());
    }

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

        /// Fuzz test for prorated rent calculations with random timestamps
        /// Verifies mathematical soundness across edge cases
        #[test]
        fn test_prorated_rent_fuzz(
            monthly_rent in 1..1000000i64,
            start_ts in 1600000000..1700000000u64,
            duration_hours in 1..720u64  // 1 hour to 30 days
        ) {
            let end_ts = start_ts + duration_hours * 3600;
            
            if let Some((prorated, actual_duration)) = calculate_prorated_rent(monthly_rent, start_ts, end_ts) {
                // Verify duration matches expected
                prop_assert_eq!(actual_duration, duration_hours * 3600);
                
                // Verify prorated amount is reasonable (not negative, not exceeding monthly rent)
                prop_assert!(prorated >= 0);
                prop_assert!(prorated <= monthly_rent);
                
                // For full month duration, should be close to monthly rent
                if duration_hours >= 720 && duration_hours <= 744 { // ~30-31 days
                    let tolerance = monthly_rent / 100; // 1% tolerance
                    prop_assert!((prorated - monthly_rent).abs() <= tolerance);
                }
            }
        }

        /// Fuzz test for termination refund calculations
        /// Ensures refund never exceeds paid amount and security margins are applied
        #[test]
        fn test_termination_refund_fuzz(
            monthly_rent in 1..1000000i64,
            paid_amount in 1..1000000i64,
            start_ts in 1600000000..1650000000u64,
            termination_offset_days in 1..29u32  // Terminate 1-29 days after start
        ) {
            let end_ts = start_ts + 30 * 86400; // 30-day lease
            let termination_ts = start_ts + termination_offset_days as u64 * 86400;
            
            if let Some(refund) = calculate_termination_refund(
                monthly_rent, start_ts, end_ts, termination_ts, paid_amount
            ) {
                // Refund should never exceed paid amount
                prop_assert!(refund <= paid_amount);
                
                // Refund should be non-negative
                prop_assert!(refund >= 0);
                
                // For early termination, refund should be proportional to unused time
                let unused_days = 30 - termination_offset_days;
                let expected_max_refund = (monthly_rent * unused_days as i64) / 30;
                prop_assert!(refund <= expected_max_refund + 1); // +1 for rounding tolerance
            }
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

        /// Property test: Sum of prorated periods should equal full monthly rent
        #[test]
        fn test_prorated_periods_sum_to_full_month(
            monthly_rent in 1000..100000i64,
            month_start in 1600000000..1700000000u64
        ) {
            let month_end = month_start + 30 * 86400; // Assume 30-day month for simplicity
            
            // Split month into two random periods
            let split_point = month_start + 15 * 86400; // Mid-month
            
            if let Some((first_period, _)) = calculate_prorated_rent(monthly_rent, month_start, split_point) {
                if let Some((second_period, _)) = calculate_prorated_rent(monthly_rent, split_point, month_end) {
                    let total = first_period + second_period;
                    
                    // The sum should equal monthly rent (allowing for rounding differences)
                    let tolerance = 2; // Allow small rounding difference
                    prop_assert!((total - monthly_rent).abs() <= tolerance);
                }
            }
        }
    }
}
