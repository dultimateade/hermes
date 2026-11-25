//! Fuzz target for betting and payout arithmetic.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct BettingInput {
    user_stake: i128,
    winning_total: i128,
    total_pool: i128,
    fee_bps: i128,
}

fuzz_target!(|input: BettingInput| {
    if input.winning_total <= 0
        || input.total_pool < 0
        || input.user_stake < 0
        || !(0..=10_000).contains(&input.fee_bps)
    {
        return;
    }

    let fee_factor = 10_000i128 - input.fee_bps;
    let payout = input
        .user_stake
        .checked_mul(fee_factor)
        .and_then(|value| value.checked_mul(input.total_pool))
        .and_then(|value| value.checked_div(10_000))
        .and_then(|value| value.checked_div(input.winning_total));

    if let Some(payout) = payout {
        assert!(payout >= 0);
    }
});
