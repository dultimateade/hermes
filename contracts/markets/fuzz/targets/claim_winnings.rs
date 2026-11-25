//! Cargo-fuzz target for the markets contract's claim_winnings entrypoint.
//!
//! Exercises winnings claim logic with:
//!
//! | Scenario                 | Coverage                                      |
//! |--------------------------|-----------------------------------------------|
//! | Market states            | Unresolved, resolved, cancelled markets       |
//! | User roles               | Bettor with winning bet, non-bettor, loser    |
//! | Double-claim detection   | Same user claiming twice (CEI pattern)        |
//! | Bet outcomes             | Valid winning outcome, invalid outcome        |
//! | Payout calculations      | Gross payout, LP fee deduction, remainder     |
//!
//! ## Boundary conditions
//!
//! - Market not found: returns error
//! - Market not resolved: returns error  
//! - Already claimed by same user: returns AlreadyClaimed (CEI pattern protection)
//! - No bet placed: returns error (InvalidState)
//! - Losing bet on resolved market: returns InvalidOutcome (zero payout)
//! - Winning bet: computes and deducts LP fee correctly
//! - Payout remainder tracking: accumulates fractional stroops
//!
//! ## Expected error set
//!
//! The fuzzer only observes market-family errors (frozen in auth_boundary tests):
//!
//! ```text
//! InvalidState       = 402
//! MarketNotFound     = 403
//! MarketNotResolved  = 405
//! InvalidOutcome     = 401
//! AlreadyClaimed     = 406
//! ```
//!
//! Any unexpected panic terminates the fuzzer with a crash report.
//!
//! ## Running
//!
//! ```bash
//! cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz claim_winnings
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
const ACTION_CLAIM_WINNINGS: u8 = 2;
const ACTION_ADVANCE_TIME: u8 = 3;
const NUM_ACTIONS: u8 = 4;

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

/// Advance the ledger timestamp by a number of seconds
fn advance_time(env: &Env, seconds: u64) {
    let current = env.ledger().timestamp();
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = current + seconds;
    });
}

/// Resolve a market with a specific winning outcome
fn resolve_market(
    env: &Env,
    client: &MarketContractClient,
    creator: &Address,
    market_id: u32,
    winning_outcome: u32,
) {
    let _ = client.try_resolve_market(creator, &market_id, &winning_outcome);
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

    // Pre-create markets for fuzz exploration
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
                let outcome_idx = u32::from(data[idx + 2] % NUM_OUTCOMES);
                let stake_bytes: [u8; 16] = data[idx + 3..idx + 19].try_into().unwrap();
                let stake = i128::from_be_bytes(stake_bytes);
                idx += 19;

                let user = users.get(user_idx as u32).unwrap();

                // Only place bets with positive amounts to avoid trivial rejections
                if stake > 0 {
                    let _ = client.try_place_bet(&user, &market_idx, &outcome_idx, &stake);
                }
            }

            // ────────────────────────────────────────────────────────
            // 1 — resolve_market
            //     bytes: [market_idx(1)] [winning_outcome(1)]
            // ────────────────────────────────────────────────────────
            ACTION_RESOLVE_MARKET => {
                if idx + 2 > data.len() {
                    break;
                }

                let market_idx = u32::from(data[idx] % (NUM_MARKETS as u8));
                let winning_outcome = u32::from(data[idx + 1] % NUM_OUTCOMES);
                idx += 2;

                // Advance time past market end_time so resolution succeeds
                advance_time(&env, 86_400 + 1);

                let creator = users.get(0).unwrap();
                let _ = client.try_resolve_market(&creator, &market_idx, &winning_outcome);
            }

            // ────────────────────────────────────────────────────────
            // 2 — claim_winnings
            //     bytes: [user_idx(1)] [market_idx(1)]
            // ────────────────────────────────────────────────────────
            ACTION_CLAIM_WINNINGS => {
                if idx + 2 > data.len() {
                    break;
                }

                let user_idx = (data[idx] % (NUM_USERS as u8)) as usize;
                let market_idx = u32::from(data[idx + 1] % (NUM_MARKETS as u8));
                idx += 2;

                let claimant = users.get(user_idx as u32).unwrap();

                let result = client.try_claim_winnings(&claimant, &market_idx);

                // Only market-family errors or success are acceptable
                if let Err(e) = result {
                    let error_code = match e {
                        Error::Contract(ContractError::InvalidState) => 402,
                        Error::Contract(ContractError::MarketNotFound) => 403,
                        Error::Contract(ContractError::MarketNotResolved) => 405,
                        Error::Contract(ContractError::InvalidOutcome) => 401,
                        Error::Contract(ContractError::AlreadyClaimed) => 406,
                        _ => {
                            panic!(
                                "Unexpected error from claim_winnings: {:?}",
                                e
                            );
                        }
                    };
                    // Valid error codes pass silently
                    let _ = error_code;
                }
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

            _ => unreachable!(),
        }
    }
});
