//! Tests for the runtime-configurable outcome cap (issue #009).
//!
//! The cap used to be a constant compiled into the contract, so changing it
//! needed an upgrade. It now lives in persistent storage
//! (`DataKey::MaxOutcomesPerMarket`) and is enforced on both write paths that
//! can change a market's outcome list: `create_market` and
//! `update_event_outcomes`.
//!
//! ## Coverage matrix
//!
//! | Scenario | Expected result |
//! |---|---|
//! | Cap never configured | [`DEFAULT_MAX_OUTCOMES_PER_MARKET`] (`10`) |
//! | Outcomes equal to the cap | accepted |
//! | Outcomes one over the cap | [`Error::TooManyOutcomes`] |
//! | Admin raises the cap | larger markets accepted |
//! | Admin lowers the cap | existing markets untouched, new larger markets rejected |
//! | Non-admin sets the cap | [`Error::Unauthorized`] |
//! | Cap below [`MIN_OUTCOMES_PER_MARKET`] | [`Error::InvalidInput`] |
//! | Cap above [`MAX_OUTCOMES_PER_MARKET_CEILING`] | [`Error::InvalidInput`] |
//! | `create_market` over the cap | [`Error::TooManyOutcomes`] |
//! | `update_event_outcomes` over the cap | [`Error::TooManyOutcomes`] |
//! | Cap change | `mxoutc` event published |
//!
//! ## Running
//!
//! ```bash
//! cargo test -p predictify-hybrid -- max_outcomes_cap_tests
//! ```

#![cfg(test)]

extern crate std;

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, String, Symbol, TryIntoVal, Vec,
};

use crate::err::Error;
use crate::markets::{
    MarketStateManager, OutcomeCapManager, DEFAULT_MAX_OUTCOMES_PER_MARKET,
    MAX_OUTCOMES_PER_MARKET_CEILING, MIN_OUTCOMES_PER_MARKET,
};
use crate::types::{Market, MarketState, OracleConfig, OracleProvider};
use crate::{PredictifyHybrid, PredictifyHybridClient};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DAY_SECS: u64 = 86_400;
const START_TS: u64 = 1_735_689_600;

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

/// Initializes the contract with `admin` as the primary admin.
fn setup(env: &Env, admin: &Address) -> Address {
    env.mock_all_auths();
    env.ledger().set_timestamp(START_TS);

    let contract_id = env.register(PredictifyHybrid, ());
    client(env, &contract_id).initialize(admin, &Some(200i128), &None);
    contract_id
}

/// Builds `count` distinct outcome labels (`outcome_1` .. `outcome_N`).
fn outcomes_of(env: &Env, count: u32) -> Vec<String> {
    let mut outcomes = Vec::new(env);
    for index in 0..count {
        outcomes.push_back(String::from_str(
            env,
            std::format!("outcome_{}", index + 1).as_str(),
        ));
    }
    outcomes
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

/// Runs `f` inside the contract context so storage calls resolve.
fn in_contract<T>(env: &Env, contract_id: &Address, f: impl FnOnce() -> T) -> T {
    env.as_contract(contract_id, f)
}

/// Stores a market record directly, bypassing the fee-paying creation path.
fn store_market(env: &Env, contract_id: &Address, market_id: &Symbol, outcomes: Vec<String>) {
    let market = Market::new(
        env,
        Address::generate(env),
        String::from_str(env, "Will BTC close above $100k?"),
        outcomes,
        START_TS + 30 * DAY_SECS,
        oracle_config(env),
        None,
        86_400u64,
        MarketState::Active,
    );
    in_contract(env, contract_id, || {
        MarketStateManager::update_market(env, market_id, &market);
    });
}

/// Returns the number of events published under the given topic.
fn count_events_with_topic(env: &Env, topic: Symbol) -> u32 {
    let mut count = 0;
    for event in env.events().all().iter() {
        let topics = event.1;
        if topics.is_empty() {
            continue;
        }
        let topic0: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
        if topic0 == topic {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Defaults and boundaries
// ---------------------------------------------------------------------------

#[test]
fn test_default_cap_is_ten() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    assert_eq!(
        client(&env, &contract_id).get_max_outcomes_per_market(),
        DEFAULT_MAX_OUTCOMES_PER_MARKET
    );
    assert_eq!(DEFAULT_MAX_OUTCOMES_PER_MARKET, 10);
}

#[test]
fn test_outcomes_at_the_cap_are_accepted() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let outcomes = outcomes_of(&env, DEFAULT_MAX_OUTCOMES_PER_MARKET);

    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes),
            Ok(())
        );
    });
}

#[test]
fn test_outcomes_above_the_default_cap_are_rejected() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let outcomes = outcomes_of(&env, DEFAULT_MAX_OUTCOMES_PER_MARKET + 1);

    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes),
            Err(Error::TooManyOutcomes)
        );
    });
}

#[test]
fn test_single_outcome_is_rejected() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let outcomes = outcomes_of(&env, MIN_OUTCOMES_PER_MARKET - 1);

    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes),
            Err(Error::InvalidOutcomes)
        );
    });
}

// ---------------------------------------------------------------------------
// Admin surface
// ---------------------------------------------------------------------------

#[test]
fn test_admin_can_raise_the_cap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    client(&env, &contract_id).set_max_outcomes_per_market(&admin, &12u32);
    assert_eq!(client(&env, &contract_id).get_max_outcomes_per_market(), 12);

    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes_of(&env, 12)),
            Ok(()),
            "12 outcomes must be accepted once the cap is raised to 12"
        );
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes_of(&env, 13)),
            Err(Error::TooManyOutcomes)
        );
    });
}

#[test]
fn test_admin_can_lower_the_cap_without_affecting_existing_markets() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    // A five-outcome market created while the cap was 10.
    let market_id = Symbol::new(&env, "existing");
    let existing_outcomes = outcomes_of(&env, 5);
    store_market(&env, &contract_id, &market_id, existing_outcomes.clone());

    client(&env, &contract_id).set_max_outcomes_per_market(&admin, &3u32);
    assert_eq!(client(&env, &contract_id).get_max_outcomes_per_market(), 3);

    // New markets above the lowered cap are rejected...
    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes_of(&env, 5)),
            Err(Error::TooManyOutcomes)
        );
    });

    // ...but the live market keeps its outcomes: lowering the cap must never
    // invalidate a market whose bets would then be unresolvable.
    let stored = in_contract(&env, &contract_id, || {
        MarketStateManager::get_market(&env, &market_id).expect("market must still exist")
    });
    assert_eq!(stored.outcomes, existing_outcomes);
}

#[test]
fn test_non_admin_cannot_set_the_cap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let stranger = Address::generate(&env);

    let result = client(&env, &contract_id).try_set_max_outcomes_per_market(&stranger, &12u32);
    assert_contract_error(result, Error::Unauthorized);
    assert_eq!(
        client(&env, &contract_id).get_max_outcomes_per_market(),
        DEFAULT_MAX_OUTCOMES_PER_MARKET,
        "a rejected update must not change the stored cap"
    );
}

#[test]
fn test_cap_below_minimum_is_rejected() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    let result = client(&env, &contract_id)
        .try_set_max_outcomes_per_market(&admin, &(MIN_OUTCOMES_PER_MARKET - 1));
    assert_contract_error(result, Error::InvalidInput);
    assert_eq!(
        client(&env, &contract_id).get_max_outcomes_per_market(),
        DEFAULT_MAX_OUTCOMES_PER_MARKET
    );
}

#[test]
fn test_cap_above_ceiling_is_rejected() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    let result = client(&env, &contract_id)
        .try_set_max_outcomes_per_market(&admin, &(MAX_OUTCOMES_PER_MARKET_CEILING + 1));
    assert_contract_error(result, Error::InvalidInput);
    assert_eq!(
        client(&env, &contract_id).get_max_outcomes_per_market(),
        DEFAULT_MAX_OUTCOMES_PER_MARKET
    );
}

#[test]
fn test_cap_change_emits_event() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    assert_eq!(count_events_with_topic(&env, symbol_short!("mxoutc")), 0);
    client(&env, &contract_id).set_max_outcomes_per_market(&admin, &15u32);
    assert_eq!(count_events_with_topic(&env, symbol_short!("mxoutc")), 1);
}

// ---------------------------------------------------------------------------
// Enforcement on the write paths
// ---------------------------------------------------------------------------

#[test]
fn test_create_market_rejects_outcomes_above_the_cap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);

    let result = client(&env, &contract_id).try_create_market(
        &admin,
        &String::from_str(&env, "Which team wins the league?"),
        &outcomes_of(&env, DEFAULT_MAX_OUTCOMES_PER_MARKET + 1),
        &30u32,
        &oracle_config(&env),
        &None,
        &86_400u64,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_contract_error(result, Error::TooManyOutcomes);
}

#[test]
fn test_create_market_accepts_outcomes_at_the_cap_boundary() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let outcomes = outcomes_of(&env, DEFAULT_MAX_OUTCOMES_PER_MARKET);

    // The cap is the last check before the creation fee is taken, so a market
    // with exactly `cap` outcomes must pass outcome validation.
    in_contract(&env, &contract_id, || {
        assert_eq!(
            OutcomeCapManager::validate_outcome_count(&env, &outcomes),
            Ok(())
        );
    });
}

#[test]
fn test_update_event_outcomes_cannot_bypass_the_cap() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = setup(&env, &admin);
    let market_id = Symbol::new(&env, "growable");
    store_market(&env, &contract_id, &market_id, outcomes_of(&env, 2));

    let result = client(&env, &contract_id).try_update_event_outcomes(
        &admin,
        &market_id,
        &outcomes_of(&env, DEFAULT_MAX_OUTCOMES_PER_MARKET + 1),
    );

    assert_contract_error(result, Error::TooManyOutcomes);
}
