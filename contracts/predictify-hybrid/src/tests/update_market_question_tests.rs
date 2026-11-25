//! Tests for the creator-gated `update_market_question` entrypoint (issue #004).
//!
//! A market creator may correct the question text only while the market is
//! still `Active`, before `end_time`, and while `total_staked == 0`. Those
//! conditions keep the edit harmless: nobody can have committed funds against
//! text that later changes.
//!
//! ## Coverage matrix
//!
//! | Scenario | Expected result |
//! |---|---|
//! | Creator edits before any stake | `Ok(())`, question + commitment updated |
//! | Edit after a stake exists | [`Error::InvalidState`] |
//! | Edit after a vote exists | [`Error::InvalidState`] |
//! | Edit by a non-creator | [`Error::Unauthorized`] |
//! | Edit after `end_time` | [`Error::MarketClosed`] |
//! | Edit on a non-active market | [`Error::InvalidState`] |
//! | Empty question | [`Error::InvalidQuestion`] |
//! | Question longer than [`MAX_QUESTION_LENGTH`] | [`Error::QuestionTooLong`] |
//! | Question exactly at the limit | `Ok(())` |
//!
//! ## Running
//!
//! ```bash
//! cargo test -p predictify-hybrid -- update_market_question_tests
//! ```

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Symbol, Vec,
};

use crate::err::Error;
use crate::markets::{MarketStateManager, MAX_QUESTION_LENGTH};
use crate::types::{Market, MarketState, OracleConfig, OracleProvider};
use crate::{PredictifyHybrid, PredictifyHybridClient};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DAY_SECS: u64 = 86_400;

/// Returns a client bound to a registered contract instance.
fn client<'a>(env: &'a Env, contract_id: &Address) -> PredictifyHybridClient<'a> {
    PredictifyHybridClient::new(env, contract_id)
}

/// Asserts that the invocation failed with the given contract error.
fn assert_contract_error<T: core::fmt::Debug>(
    result: Result<T, Result<soroban_sdk::Error, soroban_sdk::InvokeError>>,
    expected: Error,
) {
    match result {
        Err(Ok(actual)) => assert_eq!(
            actual,
            soroban_sdk::Error::from_contract_error(expected as u32),
            "unexpected contract error"
        ),
        other => panic!("expected contract error {:?}, got {:?}", expected, other),
    }
}

/// Minimal valid oracle configuration (Reflector, BTC/USD).
fn oracle_config(env: &Env) -> OracleConfig {
    OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: Address::from_str(
            env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ),
        feed_id: String::from_str(env, "BTC/USD"),
        threshold: 50_000i128,
        comparison: String::from_str(env, "gt"),
    }
}

/// Initializes the contract and creates a 30-day market owned by `admin`.
fn setup_market(env: &Env, admin: &Address) -> (Address, Symbol, Vec<String>) {
    env.mock_all_auths();
    env.ledger().set_timestamp(1_735_689_600);

    let contract_id = env.register(PredictifyHybrid, ());
    client(env, &contract_id).initialize(admin, &Some(200i128), &None);

    let outcomes = Vec::from_array(
        env,
        [String::from_str(env, "yes"), String::from_str(env, "no")],
    );

    let market_id = client(env, &contract_id).create_market(
        admin,
        &String::from_str(env, "Will BTC close above $100k?"),
        &outcomes,
        &30u32,
        &oracle_config(env),
        &None,
        &86_400u64,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (contract_id, market_id, outcomes)
}

/// Loads the stored market record.
fn load_market(env: &Env, contract_id: &Address, market_id: &Symbol) -> Market {
    env.as_contract(contract_id, || {
        MarketStateManager::get_market(env, market_id).expect("market must exist")
    })
}

/// Rewrites the stored market record (used to simulate states the contract
/// cannot yet reach through public entrypoints).
fn store_market(env: &Env, contract_id: &Address, market_id: &Symbol, market: &Market) {
    env.as_contract(contract_id, || {
        MarketStateManager::update_market(env, market_id, market);
    });
}

/// Builds a question string of `len` ASCII characters.
fn question_of_len(env: &Env, len: u32) -> String {
    let base = "x".repeat(len as usize);
    String::from_str(env, base.as_str())
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[test]
fn test_creator_updates_question_before_any_stake() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, outcomes) = setup_market(&env, &admin);
    let updated = String::from_str(&env, "Will BTC close above $120k?");

    let before = load_market(&env, &contract_id, &market_id);

    client(&env, &contract_id).update_market_question(&admin, &market_id, &updated);

    let after = load_market(&env, &contract_id, &market_id);
    assert_eq!(after.question, updated);
    assert_eq!(after.admin, before.admin);
    assert_eq!(after.outcomes, outcomes);
    assert_eq!(after.total_staked, 0);
    assert_ne!(
        after.metadata_commitment, before.metadata_commitment,
        "metadata commitment must be refreshed with the new question"
    );
    assert_eq!(
        after.metadata_commitment,
        Market::compute_metadata_commitment(&env, &updated, &outcomes, &after.oracle_config),
        "stored commitment must match the recomputed canonical commitment"
    );
}

#[test]
fn test_update_accepts_question_at_max_length() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);
    let updated = question_of_len(&env, MAX_QUESTION_LENGTH);

    client(&env, &contract_id).update_market_question(&admin, &market_id, &updated);

    assert_eq!(load_market(&env, &contract_id, &market_id).question, updated);
}

// ---------------------------------------------------------------------------
// Stake / vote gates
// ---------------------------------------------------------------------------

#[test]
fn test_update_rejected_after_stake() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    let mut market = load_market(&env, &contract_id, &market_id);
    market.total_staked = 1;
    store_market(&env, &contract_id, &market_id, &market);

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &String::from_str(&env, "Edited after a stake"),
    );

    assert_contract_error(result, Error::InvalidState);
}

#[test]
fn test_update_rejected_after_vote_exists() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    // Defensive gate: a vote entry (even with inconsistent total bookkeeping)
    // must also freeze the question text.
    let mut market = load_market(&env, &contract_id, &market_id);
    market.votes.set(admin.clone(), String::from_str(&env, "yes"));
    store_market(&env, &contract_id, &market_id, &market);

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &String::from_str(&env, "Edited after a vote"),
    );

    assert_contract_error(result, Error::InvalidState);
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[test]
fn test_update_rejected_for_non_creator() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);
    let stranger = Address::generate(&env);

    let result = client(&env, &contract_id).try_update_market_question(
        &stranger,
        &market_id,
        &String::from_str(&env, "Not my market"),
    );

    assert_contract_error(result, Error::Unauthorized);
}

// ---------------------------------------------------------------------------
// Time / state gates
// ---------------------------------------------------------------------------

#[test]
fn test_update_rejected_after_end_time() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    env.ledger().with_mut(|ledger| {
        ledger.timestamp += 31 * DAY_SECS;
    });

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &String::from_str(&env, "Too late"),
    );

    assert_contract_error(result, Error::MarketClosed);
}

#[test]
fn test_update_rejected_when_market_not_active() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    let mut market = load_market(&env, &contract_id, &market_id);
    market.state = MarketState::Resolved;
    store_market(&env, &contract_id, &market_id, &market);

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &String::from_str(&env, "Resolved market"),
    );

    assert_contract_error(result, Error::InvalidState);
}

// ---------------------------------------------------------------------------
// Question validation
// ---------------------------------------------------------------------------

#[test]
fn test_update_rejected_for_empty_question() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &String::from_str(&env, ""),
    );

    assert_contract_error(result, Error::InvalidQuestion);
}

#[test]
fn test_update_rejected_for_overlong_question() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (contract_id, market_id, _) = setup_market(&env, &admin);

    let result = client(&env, &contract_id).try_update_market_question(
        &admin,
        &market_id,
        &question_of_len(&env, MAX_QUESTION_LENGTH + 1),
    );

    assert_contract_error(result, Error::QuestionTooLong);
    assert_eq!(
        load_market(&env, &contract_id, &market_id).question,
        String::from_str(&env, "Will BTC close above $100k?"),
        "a rejected update must not modify the stored question"
    );
}
