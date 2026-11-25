//! Cargo-fuzz target for the Monitor contract — cap enforcement invariants.
//!
//! # What this target proves
//!
//! The Monitor contract maintains per-account counters for three resource
//! types (bets, positions, subscriptions) and enforces a configurable cap on
//! each one.  Two invariants must hold after **every** operation, regardless
//! of the order in which `record_*` and `remove_*` calls are interleaved:
//!
//! 1. **No counter falls below zero** (`count >= 0`).  Enforced in the source
//!    by `checked_sub` returning `MonitorError::Underflow` on zero, but an
//!    off-by-one in that guard would produce a wrap-around to `u32::MAX`.
//!
//! 2. **No counter exceeds its cap** (`count <= cap`).  Enforced by
//!    `check_*_cap` using `current >= max`.  An off-by-one (`>` instead of
//!    `>=`) would let the count reach `cap + 1`.
//!
//! These properties cannot both be violated at the same time, but each one
//! covers a distinct class of `>=` / `>` confusion in the enforcement code.
//!
//! # Fuzz action encoding
//!
//! Each action consumes a fixed number of bytes so the fuzzer can mutate the
//! corpus structurally without an explicit grammar.
//!
//! | Action byte (mod 9) | Meaning                        | Extra bytes |
//! |---------------------|--------------------------------|-------------|
//! | 0                   | `record_bet(user)`             | 1 (user idx)|
//! | 1                   | `remove_bet(user)`             | 1           |
//! | 2                   | `record_position(user)`        | 1           |
//! | 3                   | `remove_position(user)`        | 1           |
//! | 4                   | `record_subscription(user)`    | 1           |
//! | 5                   | `remove_subscription(user)`    | 1           |
//! | 6                   | `set_caps(Bets, new_max)`      | 1 (1-byte cap, 1..=255 range)|
//! | 7                   | `set_caps(Positions, new_max)` | 1           |
//! | 8                   | `set_caps(Subscriptions, new_max)` | 1       |
//!
//! After every mutating action the target reads back `get_account_state` and
//! `get_caps` and asserts both invariants.  Any assertion failure or panic
//! terminates the fuzzer with a crash report.
//!
//! # Running with cargo-fuzz (requires nightly + cargo-fuzz)
//!
//! ```bash
//! cargo +nightly fuzz run --fuzz-dir contracts/monitor/fuzz cap_sequence
//! ```
//!
//! # Running as a plain deterministic test (no nightly required)
//!
//! The target also compiles under `cargo test -p monitor-fuzz` because it
//! depends only on `monitor` with the `testutils` feature of `soroban-sdk`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};
use monitor::{AccountState, CapType, MonitorContract, MonitorContractClient, MonitorError};

// ---------------------------------------------------------------------------
// Number of distinct user addresses available to the fuzzer.
// Three is enough to exercise per-account isolation without slowing corpus
// generation; keeping it small makes interesting sequences more likely.
// ---------------------------------------------------------------------------
const NUM_USERS: usize = 3;

// ---------------------------------------------------------------------------
// Number of distinct action opcodes.
// ---------------------------------------------------------------------------
const NUM_ACTIONS: u8 = 9;

// ---------------------------------------------------------------------------
// Cap values injected by set_caps actions are taken from the raw byte value
// (1–255) so they stay within the valid range (1 ..= HARD_UPPER_BOUND).
// Zero would trigger InvalidInput; values above HARD_UPPER_BOUND would too.
// We clamp to 1..=255 which is well inside the 10 000 hard limit.
// ---------------------------------------------------------------------------
const CAP_BYTE_MIN: u32 = 1;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ledger_info() -> LedgerInfo {
    LedgerInfo {
        timestamp: 1_700_000_000,
        protocol_version: 20,
        sequence_number: 100,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 535_680,
    }
}

/// Assert that every user's account state respects both invariants:
///   - each counter is in `0 ..= cap` for its resource type.
///
/// Any violation panics, which the fuzzer treats as a crash.
fn assert_invariants(client: &MonitorContractClient, users: &[Address]) {
    let caps = client.get_caps();

    for user in users {
        // An account that has never had a successful record_* call has no
        // persistent state. get_account_state returns None in that case;
        // treat it as all-zero counters, which trivially satisfies both
        // invariants.
        let state = client
            .get_account_state(user)
            .unwrap_or(AccountState {
                bets: 0,
                positions: 0,
                subscriptions: 0,
            });

        assert!(
            state.bets <= caps.max_bets,
            "INVARIANT VIOLATED: bets counter {} exceeds cap {} for user {:?}",
            state.bets,
            caps.max_bets,
            user,
        );

        assert!(
            state.positions <= caps.max_positions,
            "INVARIANT VIOLATED: positions counter {} exceeds cap {} for user {:?}",
            state.positions,
            caps.max_positions,
            user,
        );

        assert!(
            state.subscriptions <= caps.max_subscriptions,
            "INVARIANT VIOLATED: subscriptions counter {} exceeds cap {} for user {:?}",
            state.subscriptions,
            caps.max_subscriptions,
            user,
        );

        // u32 can never be negative in Rust — but if the underflow guard were
        // broken (e.g. saturating instead of checked) the counter could wrap to
        // u32::MAX.  Treat any value > HARD_UPPER_BOUND as evidence of wrap.
        const WRAP_SENTINEL: u32 = 10_001;
        assert!(
            state.bets < WRAP_SENTINEL,
            "INVARIANT VIOLATED: bets counter {} looks like a wrapped underflow",
            state.bets,
        );
        assert!(
            state.positions < WRAP_SENTINEL,
            "INVARIANT VIOLATED: positions counter {} looks like a wrapped underflow",
            state.positions,
        );
        assert!(
            state.subscriptions < WRAP_SENTINEL,
            "INVARIANT VIOLATED: subscriptions counter {} looks like a wrapped underflow",
            state.subscriptions,
        );
    }
}

// ---------------------------------------------------------------------------
// Main fuzz target
// ---------------------------------------------------------------------------

fuzz_target!(|data: &[u8]| {
    // Skip trivially empty inputs — nothing to exercise.
    if data.is_empty() {
        return;
    }

    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());

    // -----------------------------------------------------------------------
    // Setup: register and initialize the contract, generate user pool.
    // -----------------------------------------------------------------------
    let admin = Address::generate(&env);
    let users: [Address; NUM_USERS] = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    // -----------------------------------------------------------------------
    // Drive fuzz loop.
    // -----------------------------------------------------------------------
    let mut idx = 0;

    while idx < data.len() {
        let action = data[idx] % NUM_ACTIONS;
        idx += 1;

        // Every action that uses a user index or a cap byte needs 1 extra byte.
        if idx >= data.len() {
            break;
        }
        let param_byte = data[idx];
        idx += 1;

        let user = &users[(param_byte % NUM_USERS as u8) as usize];

        match action {
            // ----------------------------------------------------------------
            // 0 — record_bet
            // ----------------------------------------------------------------
            0 => {
                let result = client.try_record_bet(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::BetCapExceeded)) => {}
                    Ok(Err(e)) => panic!("record_bet: unexpected error {:?}", e),
                    Err(e) => panic!("record_bet: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 1 — remove_bet
            // ----------------------------------------------------------------
            1 => {
                let result = client.try_remove_bet(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::Underflow)) => {}
                    Ok(Err(e)) => panic!("remove_bet: unexpected error {:?}", e),
                    Err(e) => panic!("remove_bet: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 2 — record_position
            // ----------------------------------------------------------------
            2 => {
                let result = client.try_record_position(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::PositionCapExceeded)) => {}
                    Ok(Err(e)) => panic!("record_position: unexpected error {:?}", e),
                    Err(e) => panic!("record_position: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 3 — remove_position
            // ----------------------------------------------------------------
            3 => {
                let result = client.try_remove_position(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::Underflow)) => {}
                    Ok(Err(e)) => panic!("remove_position: unexpected error {:?}", e),
                    Err(e) => panic!("remove_position: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 4 — record_subscription
            // ----------------------------------------------------------------
            4 => {
                let result = client.try_record_subscription(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::SubscriptionCapExceeded)) => {}
                    Ok(Err(e)) => panic!("record_subscription: unexpected error {:?}", e),
                    Err(e) => panic!("record_subscription: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 5 — remove_subscription
            // ----------------------------------------------------------------
            5 => {
                let result = client.try_remove_subscription(user);
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(MonitorError::Underflow)) => {}
                    Ok(Err(e)) => panic!("remove_subscription: unexpected error {:?}", e),
                    Err(e) => panic!("remove_subscription: SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 6 — set_caps(Bets, cap)
            //
            // Map the raw byte to a valid cap in 1..=255.  Zero would return
            // InvalidInput (also acceptable), but keeping values valid lets
            // the fuzzer spend more time exploring counter sequences rather
            // than admin-validation paths.
            // ----------------------------------------------------------------
            6 => {
                let new_max = (param_byte as u32).max(CAP_BYTE_MIN);
                let result = client.try_set_caps(&admin, &CapType::Bets, &new_max);
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(MonitorError::InvalidInput)) => {}
                    Ok(Err(e)) => panic!("set_caps(Bets): unexpected error {:?}", e),
                    Err(e) => panic!("set_caps(Bets): SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 7 — set_caps(Positions, cap)
            // ----------------------------------------------------------------
            7 => {
                let new_max = (param_byte as u32).max(CAP_BYTE_MIN);
                let result = client.try_set_caps(&admin, &CapType::Positions, &new_max);
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(MonitorError::InvalidInput)) => {}
                    Ok(Err(e)) => panic!("set_caps(Positions): unexpected error {:?}", e),
                    Err(e) => panic!("set_caps(Positions): SDK error {:?}", e),
                }
            }

            // ----------------------------------------------------------------
            // 8 — set_caps(Subscriptions, cap)
            // ----------------------------------------------------------------
            8 => {
                let new_max = (param_byte as u32).max(CAP_BYTE_MIN);
                let result = client.try_set_caps(&admin, &CapType::Subscriptions, &new_max);
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(MonitorError::InvalidInput)) => {}
                    Ok(Err(e)) => panic!("set_caps(Subscriptions): unexpected error {:?}", e),
                    Err(e) => panic!("set_caps(Subscriptions): SDK error {:?}", e),
                }
            }

            _ => unreachable!(),
        }

        // Assert both invariants after every operation.
        assert_invariants(&client, &users);
    }
});
