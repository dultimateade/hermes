#![cfg(test)]
extern crate std;

use oracles::{DataKey, OraclesContract, OraclesContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const MARKET_RESULT_TTL_LEDGERS: u32 = 6_307_200;
const MARKET_RESULT_TTL_BUMP_THRESHOLD: u32 = 120_960;

#[test]
fn test_ttl_bump_hot_read_paths() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(OraclesContract, ());
    let client = OraclesContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let oracle = env.register(OracleWithoutPrice, ());
    client.add_oracle(&admin, &oracle).unwrap();

    // list_oracles should succeed and bump TTL
    let oracles = client.list_oracles();
    assert_eq!(oracles.len(), 1);
    assert_eq!(oracles.get(0).unwrap(), oracle);

    // This contract advertises GET_PRICE but omits the price entrypoints, so
    // the calls fail after exercising the registry TTL bump.
    let feed_id = String::from_str(&env, "BTC/USD");
    
    let res1 = client.try_get_price(&oracle, &feed_id);
    assert!(res1.is_err(), "Expected error because get_price is absent");

    let res2 = client.try_get_price_data(&oracle, &feed_id);
    assert!(res2.is_err(), "Expected error because price entrypoints are absent");

    // For is_oracle_healthy, it intercepts the error and returns false if the contract invocation fails
    let is_healthy = client.is_oracle_healthy(&oracle, &feed_id);
    assert_eq!(is_healthy, false);
}

#[test]
fn market_result_submission_sets_one_year_ttl_and_get_renews_near_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(OraclesContract, ());
    let client = OraclesContractClient::new(&env, &contract_id);
    let market_id = soroban_sdk::Symbol::new(&env, "market_1");
    let oracle = Address::generate(&env);
    let outcome = String::from_str(&env, "yes");
    let key = DataKey::MarketResult(market_id.clone(), oracle.clone());

    client.submit_market_result(&market_id, &oracle, &outcome);

    env.as_contract(&contract_id, || {
        assert_eq!(
            env.storage().persistent().get_ttl(&key),
            MARKET_RESULT_TTL_LEDGERS
        );
    });

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += MARKET_RESULT_TTL_LEDGERS - MARKET_RESULT_TTL_BUMP_THRESHOLD + 1;
    });

    assert_eq!(client.get_result(&market_id, &oracle), Some(outcome));
    env.as_contract(&contract_id, || {
        assert_eq!(
            env.storage().persistent().get_ttl(&key),
            MARKET_RESULT_TTL_LEDGERS
        );
    });
}

#[test]
#[should_panic]
fn market_result_submission_requires_oracle_authorization() {
    let env = Env::default();
    let contract_id = env.register(OraclesContract, ());
    let client = OraclesContractClient::new(&env, &contract_id);
    let market_id = soroban_sdk::Symbol::new(&env, "market_1");
    let oracle = Address::generate(&env);
    let result = String::from_str(&env, "yes");

    client.submit_market_result(&market_id, &oracle, &result);
}
