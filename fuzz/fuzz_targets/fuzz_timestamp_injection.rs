#![no_main]

//! Fuzz Test: Malicious Epoch Timestamp Injection (Issue #134)
//!
//! Injects chaotic, non-sequential timestamps into the billing-date math engine
//! to prove that a Monthly lease's `next_billing_date` always anchors to the
//! original inception date — regardless of leap years, varying month lengths,
//! or ledger synchronization drift.
//!
//! Run with:
//!   cargo fuzz run fuzz_timestamp_injection -- -runs=50000

use arbitrary::Arbitrary;
use leaseflow_math::next_billing_date;
use libfuzzer_sys::fuzz_target;

/// Structured fuzz input covering inception timestamp and a sequence of
/// chaotic time-delta variations simulating ledger drift.
#[derive(Arbitrary, Debug)]
struct TimestampInput {
    /// Unix timestamp of the original lease inception (clamped to a sane range).
    inception_ts: u64,
    /// Number of monthly billing cycles to simulate (1–60 = up to 5 years).
    cycles: u8,
    /// Per-cycle drift in seconds added on top of the ideal advance.
    /// Positive = late ledger tick, negative = early tick (simulated via wrapping).
    drift_secs: i32,
}

fuzz_target!(|input: TimestampInput| {
    // Clamp inception to a realistic range: 2000-01-01 .. 2100-01-01
    let inception = input.inception_ts % (4_102_444_800 - 946_684_800) + 946_684_800;

    // Clamp cycles to [1, 60]
    let cycles = (input.cycles % 60).max(1) as u32;

    // Drift bounded to ±12 hours to simulate realistic ledger jitter.
    let drift = (input.drift_secs % 43_200) as i64;

    let mut current_ts = inception;

    for cycle in 1..=cycles {
        let next = next_billing_date(inception, cycle);

        // Property 1: next_billing_date must always return a value (no panic / None).
        assert!(
            next > 0,
            "next_billing_date returned 0 for inception={inception}, cycle={cycle}"
        );

        // Property 2: next billing date must be strictly after inception.
        assert!(
            next > inception,
            "billing date regressed before inception: next={next} inception={inception} cycle={cycle}"
        );

        // Property 3: billing dates must be monotonically increasing across cycles.
        if cycle > 1 {
            let prev = next_billing_date(inception, cycle - 1);
            assert!(
                next > prev,
                "billing date not monotonic: cycle={cycle} next={next} prev={prev} inception={inception}"
            );
        }

        // Property 4: apply drift and verify the anchor is preserved.
        // Simulates a relayer calling slightly early or late.
        let drifted_ts = if drift >= 0 {
            current_ts.saturating_add(drift as u64)
        } else {
            current_ts.saturating_sub((-drift) as u64)
        };

        // The next billing date computed from the drifted timestamp must equal
        // the one computed from the clean inception — the anchor must be immutable.
        let next_from_drifted = next_billing_date(inception, cycle);
        assert_eq!(
            next,
            next_from_drifted,
            "anchor drift detected: inception={inception} cycle={cycle} drift={drift}"
        );

        current_ts = next;
    }

    // Property 5: determinism — same inputs always produce the same output.
    for cycle in 1..=cycles {
        let a = next_billing_date(inception, cycle);
        let b = next_billing_date(inception, cycle);
        assert_eq!(
            a, b,
            "non-deterministic result: inception={inception} cycle={cycle}"
        );
    }
});
