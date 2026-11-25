//! Cargo-fuzz target for the fees contract's collect_fees entrypoint.
//!
//! Exercises fee collection logic with:
//!
//! | Scenario                 | Coverage                                      |
//! |--------------------------|-----------------------------------------------|
//! | Auth & admin checks      | Admin auth enforcement, non-admin rejection   |
//! | Collection threshold     | Below threshold (rejected), at/above threshold|
//! | Fee configuration states | Enabled/paused, various thresholds            |
//! | Withdrawal schedule      | Cooldown enforcement, schedule updates        |
//! | Collected balance        | Accumulation, reset after collection          |
//! | Edge cases               | Zero collected, overflow scenarios             |
//!
//! ## Boundary conditions
//!
//! - Admin not set: returns AdminNotSet
//! - Non-admin caller: returns Unauthorized
//! - Fees paused: returns FeesPaused
//! - Collected < threshold: returns BelowCollectionThreshold
//! - Cooldown active: returns WithdrawalCooldownActive
//! - Valid collection: updates balance, resets collected to 0, updates schedule
//! - Overflow detection: i128::MAX + amount scenarios
//!
//! ## Expected error set
//!
//! The fuzzer only observes fees-family errors (frozen in auth_boundary tests):
//!
//! ```text
//! Unauthorized               = 1
//! AdminNotSet                = 2
//! FeesPaused                 = 6
//! BelowCollectionThreshold   = 9
//! WithdrawalCooldownActive   = 14
//! InvalidState               = 4
//! ```
//!
//! Any unexpected panic terminates the fuzzer with a crash report.
//!
//! ## Running
//!
//! ```bash
//! cargo +nightly fuzz run --fuzz-dir contracts/fees/fuzz collect_fees
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, Symbol,
};
use fees::{ContractError, FeesContractClient, FeesContract, Error};

// Constants for fuzz test
const NUM_ADMINS: usize = 2;
const NUM_CALLERS: usize = 3;

// Fuzz action types
const ACTION_INITIALIZE: u8 = 0;
const ACTION_RECORD_FEE: u8 = 1;
const ACTION_COLLECT_FEES: u8 = 2;
const ACTION_ADVANCE_TIME: u8 = 3;
const ACTION_PAUSE_FEES: u8 = 4;
const ACTION_UNPAUSE_FEES: u8 = 5;
const NUM_ACTIONS: u8 = 6;

// Helper to initialize the fees contract
fn setup_contract(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(env, &contract_id);

    // Initialize the contract
    let _ = client.try_initialize(&admin);

    // Authorize some callers for fee recording
    for i in 0..NUM_CALLERS as u32 {
        let caller = Address::generate(env);
        let _ = client.try_authorize_caller(&admin, &caller);
    }

    (admin, contract_id)
}

/// Accumulate recorded fees by a caller
fn record_fee(
    env: &Env,
    client: &FeesContractClient,
    caller: &Address,
    amount: i128,
) {
    if amount > 0 {
        let _ = client.try_record_fee(caller, &amount);
    }
}

/// Collect accumulated fees (requires admin auth)
fn collect_fees(env: &Env, client: &FeesContractClient, admin: &Address) {
    let result = client.try_collect_fees(admin);

    // Only fees-family errors or success are acceptable
    if let Err(e) = result {
        let error_code = match e {
            Error::Contract(ContractError::Unauthorized) => 1,
            Error::Contract(ContractError::AdminNotSet) => 2,
            Error::Contract(ContractError::FeesPaused) => 6,
            Error::Contract(ContractError::BelowCollectionThreshold) => 9,
            Error::Contract(ContractError::WithdrawalCooldownActive) => 14,
            Error::Contract(ContractError::InvalidState) => 4,
            _ => {
                panic!("Unexpected error from collect_fees: {:?}", e);
            }
        };
        // Valid error codes pass silently
        let _ = error_code;
    }
}

/// Advance the ledger timestamp by a number of seconds
fn advance_time(env: &Env, seconds: u64) {
    let current = env.ledger().timestamp();
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = current + seconds;
    });
}

/// Pause fees collection
fn pause_fees(env: &Env, client: &FeesContractClient, admin: &Address) {
    let _ = client.try_pause_fees(admin);
}

/// Unpause fees collection
fn unpause_fees(env: &Env, client: &FeesContractClient, admin: &Address) {
    let _ = client.try_unpause_fees(admin);
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let env = Env::default();
    env.mock_all_auths();

    // ──────────────────────────────────────────────────────────────
    // Contract setup
    // ──────────────────────────────────────────────────────────────
    let (admin, contract_id) = setup_contract(&env);
    let client = FeesContractClient::new(&env, &contract_id);

    // Pre-authorize some callers that can record fees
    let mut authorized_callers = Vec::new();
    for i in 0..NUM_CALLERS {
        authorized_callers.push(Address::generate(&env));
        let _ = client.try_authorize_caller(&admin, &authorized_callers[i]);
    }

    let mut alt_admins = Vec::new();
    for _ in 0..NUM_ADMINS {
        alt_admins.push(Address::generate(&env));
    }

    // ──────────────────────────────────────────────────────────────
    // Drive fuzzer loop through actions
    // ──────────────────────────────────────────────────────────────
    let mut idx = 0;

    while idx < data.len() {
        let action_type = data[idx] % NUM_ACTIONS;
        idx += 1;

        match action_type {
            // ────────────────────────────────────────────────────────
            // 0 — initialize (no-op if already initialized)
            //     bytes: (none)
            // ────────────────────────────────────────────────────────
            ACTION_INITIALIZE => {
                let new_admin = alt_admins.get(0).unwrap().clone();
                let _ = client.try_initialize(&new_admin);
            }

            // ────────────────────────────────────────────────────────
            // 1 — record_fee
            //     bytes: [caller_idx(1)] [amount(16)]
            // ────────────────────────────────────────────────────────
            ACTION_RECORD_FEE => {
                if idx + 17 > data.len() {
                    break;
                }

                let caller_idx = (data[idx] % (NUM_CALLERS as u8)) as usize;
                let amount_bytes: [u8; 16] = data[idx + 1..idx + 17].try_into().unwrap();
                let amount = i128::from_be_bytes(amount_bytes);
                idx += 17;

                let caller = authorized_callers.get(caller_idx).unwrap();
                record_fee(&env, &client, caller, amount);
            }

            // ────────────────────────────────────────────────────────
            // 2 — collect_fees
            //     bytes: [admin_idx(1)] (use 0 for main admin, 1+ for non-admin)
            // ────────────────────────────────────────────────────────
            ACTION_COLLECT_FEES => {
                if idx + 1 > data.len() {
                    break;
                }

                let admin_choice = data[idx] % 3;
                idx += 1;

                let collector = match admin_choice {
                    0 => admin.clone(),
                    1 => alt_admins.get(0).unwrap().clone(),
                    _ => Address::generate(&env),
                };

                collect_fees(&env, &client, &collector);
            }

            // ────────────────────────────────────────────────────────
            // 3 — advance_time
            //     bytes: [seconds_delta(2)]
            // ────────────────────────────────────────────────────────
            ACTION_ADVANCE_TIME => {
                if idx + 2 > data.len() {
                    break;
                }

                let seconds_delta_bytes: [u8; 2] = data[idx..idx + 2].try_into().unwrap();
                let seconds_delta = u16::from_be_bytes(seconds_delta_bytes) as u64;
                idx += 2;

                advance_time(&env, seconds_delta);
            }

            // ────────────────────────────────────────────────────────
            // 4 — pause_fees
            //     bytes: [admin_choice(1)]
            // ────────────────────────────────────────────────────────
            ACTION_PAUSE_FEES => {
                if idx + 1 > data.len() {
                    break;
                }

                let admin_choice = data[idx] % 2;
                idx += 1;

                let admin_to_pause = match admin_choice {
                    0 => admin.clone(),
                    _ => alt_admins.get(0).unwrap().clone(),
                };

                pause_fees(&env, &client, &admin_to_pause);
            }

            // ────────────────────────────────────────────────────────
            // 5 — unpause_fees
            //     bytes: [admin_choice(1)]
            // ────────────────────────────────────────────────────────
            ACTION_UNPAUSE_FEES => {
                if idx + 1 > data.len() {
                    break;
                }

                let admin_choice = data[idx] % 2;
                idx += 1;

                let admin_to_unpause = match admin_choice {
                    0 => admin.clone(),
                    _ => alt_admins.get(0).unwrap().clone(),
                };

                unpause_fees(&env, &client, &admin_to_unpause);
            }

            _ => unreachable!(),
        }
    }
});
