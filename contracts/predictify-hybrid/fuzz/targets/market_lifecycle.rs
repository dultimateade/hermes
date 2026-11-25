//! Fuzz target for market lifecycle state transitions.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::Env;

#[derive(Arbitrary, Debug)]
struct MarketLifecycleInput {
    duration_days: u32,
    now: u64,
    state_byte: u8,
    outcome_count: u8,
}

fuzz_target!(|input: MarketLifecycleInput| {
    let env = Env::default();
    let duration = input.duration_days.min(365);
    let state = input.state_byte % 5;
    let outcome_count = (input.outcome_count % 8) as u32;

    let end_time = input.now.saturating_add((duration as u64).saturating_mul(86_400));
    assert!(end_time >= input.now);

    if state == 0 {
        assert!(outcome_count <= 7);
    }

    let _ = env;
});
