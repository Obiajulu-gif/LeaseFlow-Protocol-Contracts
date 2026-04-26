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
