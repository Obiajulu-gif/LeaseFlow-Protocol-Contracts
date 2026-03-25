#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use leaseflow_math::calculate_deposit_split;

/// Fuzz input for deposit splitting.
#[derive(Arbitrary, Debug)]
struct SplitInput {
    total_deposit: i128,
    landlord_bps: u32,
}

fuzz_target!(|input: SplitInput| {
    let SplitInput {
        total_deposit,
        landlord_bps,
    } = input;

    // --- Property 1: Negative deposits are not supported ---
    if total_deposit < 0 {
        return;
    }

    // --- Property 2: Total returned split must equal total deposit (NO STUCK TOKENS) ---
    match calculate_deposit_split(total_deposit, landlord_bps) {
        None => {
            // Correctly caught overflow.
            // Check if it was really an overflow.
            let landlord_pct = (landlord_bps.min(10000)) as i128;
            if total_deposit.checked_mul(landlord_pct).is_some() {
                panic!("calculate_deposit_split returned None unexpectedly for total={}, bps={}", total_deposit, landlord_bps);
            }
        }
        Some((landlord_share, tenant_share)) => {
            // Main check: No tokens are lost or created out of thin air.
            assert_eq!(
                landlord_share + tenant_share,
                total_deposit,
                "Sum of split shares ({}+{}) doesn't equal total deposit ({}) at bps={}",
                landlord_share,
                tenant_share,
                total_deposit,
                landlord_bps
            );

            // --- Property 3: Shares must be non-negative for non-negative total ---
            assert!(landlord_share >= 0, "Negative landlord share: {}", landlord_share);
            assert!(tenant_share >= 0, "Negative tenant share: {}", tenant_share);

            // --- Property 4: Determinism ---
            let again = calculate_deposit_split(total_deposit, landlord_bps).unwrap();
            assert_eq!((landlord_share, tenant_share), again);

            // --- Property 5: Monotonicity with bps ---
            // If we increase bps, landlord share should not decrease.
            if landlord_bps < 10000 {
                if let Some((l2, _)) = calculate_deposit_split(total_deposit, landlord_bps + 1) {
                    assert!(l2 >= landlord_share, "BPS monotonicity violation: bps={} gave landlord {}, bps={} gave {}", landlord_bps, landlord_share, landlord_bps+1, l2);
                }
            }
        }
    }
});
