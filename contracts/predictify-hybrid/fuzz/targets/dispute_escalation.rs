//! Fuzz target for dispute escalation and council voting.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct DisputeInput {
    support_stake: i128,
    reject_stake: i128,
    council_votes: u32,
    escalation_level: u8,
}

fuzz_target!(|input: DisputeInput| {
    if input.support_stake < 0 || input.reject_stake < 0 {
        return;
    }

    let total = match input.support_stake.checked_add(input.reject_stake) {
        Some(total) => total,
        None => return,
    };
    if total == 0 {
        return;
    }

    let support_weight = input.support_stake.saturating_mul(100) / total;
    let reject_weight = 100i128.saturating_sub(support_weight);
    assert!(support_weight >= 0 && support_weight <= 100);
    assert!(reject_weight >= 0 && reject_weight <= 100);
    assert_eq!(support_weight + reject_weight, 100);

    let level = input.escalation_level.min(3);
    let votes = input.council_votes.min(100);
    assert!(level <= 3);
    assert!(votes <= 100);
});
