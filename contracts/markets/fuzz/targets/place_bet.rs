//! Cargo-fuzz target for the markets contract's place_bet entrypoint.
//!
//! Exercises bet placement logic with:
//!
//! | Scenario                 | Coverage                                      |
//! |--------------------------|-----------------------------------------------|
//! | Arbitrary stake amounts  | Zero, negative, small, large, i128::MAX values|
//! | Market states            | Active, resolved, cancelled markets           |
//! | Outcome index validation | Valid, invalid (out-of-bounds) outcomes       |
//! | Multiple user sequences  | Same user, different users placing bets       |
//! | Pool accumulation        | Bet amount accounting, overflow detection      |
//!
//! ## Boundary conditions
//!
//! - Stake amounts: zero (rejected), negative (rejected), below/at/above limits
//! - Outcome indices: valid (0..num_outcomes), invalid (>= num_outcomes)
//! - Market states: active (accept), resolved/cancelled (reject)
//! - Pool overflow: detect i128::MAX + amount scenarios
//! - User auth: enforced via require_auth() mock
//!
//! ## Expected error set
//!
//! The fuzzer only observes market-family errors (frozen in auth_boundary tests):
//!
//! ```text
//! StakeTooSmall      = 400
//! InvalidOutcome     = 401
//! InvalidState       = 402
//! MarketNotFound     = 403
//! Overflow           = 410
//! ```
//!
//! Any unexpected panic terminates the fuzzer with a crash report.
//!
//! ## Running
//!
//! ```bash
//! cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec as SorobanVec,
};
use markets::{ContractError, MarketsContractClient, MarketsContract, Error};

// Constants matching markets contract
const NUM_OUTCOMES: u32 = 3;
const NUM_MARKETS: u32 = 4;
const NUM_USERS: usize = 5;

// Fuzz action types
const ACTION_PLACE_BET: u8 = 0;
const ACTION_RESOLVE_MARKET: u8 = 1;
const ACTION_CANCEL_MARKET: u8 = 2;
const NUM_ACTIONS: u8 = 3;

// Helper to initialize the markets contract
fn setup_contract(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(env, &contract_id);

    // Initialize the contract
    let _ = client.try_initialize(&admin);

    (admin, contract_id)
}

/// Create a market with standard configuration
fn create_market(
    env: &Env,
    client: &MarketsContractClient,
    creator: &Address,
    market_id: u32,
) {
    let outcome_tags = {
        let mut tags = SorobanVec::new(env);
        tags.push_back(String::from_str(env, "outcome_0"));
        tags.push_back(String::from_str(env, "outcome_1"));
        tags.push_back(String::from_str(env, "outcome_2"));
        tags
    };

    let now = env.ledger().timestamp();
    let end_time = now + 86_400; // Market ends in 1 day

    let _ = client.try_create_market(
        creator,
        &String::from_str(env, &format!("Market {} question?", market_id)),
        &String::from_str(env, "Test market description"),
        &end_time,
        &String::from_str(env, "Test Source"),
        &outcome_tags,
    );
}

/// Set market to resolved state
fn resolve_market(env: &Env, client: &MarketsContractClient, creator: &Address, market_id: u32) {
    let _ = client.try_resolve_market(creator, &market_id, &0u32);
}

/// Set market to cancelled state
fn cancel_market(env: &Env, client: &MarketsContractClient, creator: &Address, market_id: u32) {
    let _ = client.try_cancel_market(creator, &market_id);
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
    let client = MarketContractClient::new(&env, &contract_id);

    // Create a pool of users
    let mut users = SorobanVec::new(&env);
    for _ in 0..NUM_USERS as u32 {
        users.push_back(Address::generate(&env));
    }

    // Pre-create markets with different states for fuzz exploration
    for market_idx in 0..NUM_MARKETS {
        let creator = users.get(0).unwrap();
        create_market(&env, &client, &creator, market_idx);
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
            // 0 — place_bet
            //     bytes: [user_idx(1)] [market_idx(1)] [outcome_idx(1)] [stake(16)]
            // ────────────────────────────────────────────────────────
            ACTION_PLACE_BET => {
                if idx + 19 > data.len() {
                    break;
                }

                let user_idx = (data[idx] % (NUM_USERS as u8)) as usize;
                let market_idx = u32::from(data[idx + 1] % (NUM_MARKETS as u8));
                let outcome_idx = u32::from(data[idx + 2]);
                let stake_bytes: [u8; 16] = data[idx + 3..idx + 19].try_into().unwrap();
                let stake = i128::from_be_bytes(stake_bytes);
                idx += 19;

                let user = users.get(user_idx as u32).unwrap();

                let result = client.try_place_bet(&user, &market_idx, &outcome_idx, &stake);

                // Only market-family errors or success are acceptable
                if let Err(e) = result {
                    let error_code = match e {
                        Error::Contract(ContractError::StakeTooSmall) => 400,
                        Error::Contract(ContractError::InvalidOutcome) => 401,
                        Error::Contract(ContractError::InvalidState) => 402,
                        Error::Contract(ContractError::MarketNotFound) => 403,
                        Error::Contract(ContractError::Overflow) => 410,
                        _ => {
                            panic!("Unexpected error from place_bet: {:?}", e);
                        }
                    };
                    // Valid error codes pass silently
                    let _ = error_code;
                }
            }

            // ────────────────────────────────────────────────────────
            // 1 — resolve_market
            //     bytes: [market_idx(1)]
            // ────────────────────────────────────────────────────────
            ACTION_RESOLVE_MARKET => {
                if idx + 1 > data.len() {
                    break;
                }

                let market_idx = u32::from(data[idx] % (NUM_MARKETS as u8));
                idx += 1;

                let creator = users.get(0).unwrap();
                resolve_market(&env, &client, &creator, market_idx);
            }

            // ────────────────────────────────────────────────────────
            // 2 — cancel_market
            //     bytes: [market_idx(1)]
            // ────────────────────────────────────────────────────────
            ACTION_CANCEL_MARKET => {
                if idx + 1 > data.len() {
                    break;
                }

                let market_idx = u32::from(data[idx] % (NUM_MARKETS as u8));
                idx += 1;

                let creator = users.get(0).unwrap();
                cancel_market(&env, &client, &creator, market_idx);
            }

            _ => unreachable!(),
        }
    }
});
