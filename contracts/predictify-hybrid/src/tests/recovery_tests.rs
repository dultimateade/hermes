#![cfg(test)]

use crate::{
    err::Error,
    markets::MarketStateManager,
    recovery::{RecoveryManager, RecoveryStorage, UnclaimedWinningsPolicy},
    types::{Market, MarketState, OracleConfig, OracleProvider},
    PredictifyHybrid,
};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String, Symbol};

fn setup_contract(env: &Env) -> (Address, Address) {
    let contract_id = env.register(PredictifyHybrid, ());
    (contract_id, Address::generate(env))
}

fn make_market(env: &Env, admin: &Address) -> Market {
    let oracle_config = OracleConfig::new(
        OracleProvider::reflector(),
        Address::generate(env),
        String::from_str(env, "BTC/USD"),
        5_000_000,
        String::from_str(env, "gt"),
    );
    Market::new(
        env,
        admin.clone(),
        String::from_str(env, "Will BTC exceed $50,000?"),
        soroban_sdk::vec![
            env,
            String::from_str(env, "yes"),
            String::from_str(env, "no"),
        ],
        env.ledger().timestamp() + 86_400,
        oracle_config,
        None,
        86_400,
        MarketState::Active,
    )
}

fn set_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .set(&Symbol::new(env, "Admin"), admin);
}

#[test]
fn recovery_reconstructs_market_and_records_history() {
    let env = Env::default();
    let (contract_id, admin) = setup_contract(&env);
    let market_id = Symbol::new(&env, "recovery_ok");

    env.as_contract(&contract_id, || {
        set_admin(&env, &admin);
        let mut market = make_market(&env, &admin);
        market.stakes.set(Address::generate(&env), 125);
        market.total_staked = 100;
        MarketStateManager::update_market(&env, &market_id, &market);

        assert!(RecoveryManager::recover_market_state(&env, &admin, &market_id).unwrap());

        let recovered_market = MarketStateManager::get_market(&env, &market_id).unwrap();
        assert_eq!(recovered_market.total_staked, 125);
        assert_eq!(RecoveryStorage::history_len(&env, &market_id), 1);
        assert_eq!(
            RecoveryManager::get_recovery_status(&env, &market_id).unwrap(),
            String::from_str(&env, "recovered")
        );
    });
}

#[test]
fn expired_claim_window_is_detected_at_deadline() {
    let env = Env::default();
    let (contract_id, _admin) = setup_contract(&env);
    let market_id = Symbol::new(&env, "expired_claim");

    env.as_contract(&contract_id, || {
        let start = env.ledger().timestamp();
        UnclaimedWinningsPolicy::set_market_claim_period(&env, &market_id, 60);
        UnclaimedWinningsPolicy::set_claim_window_start_if_missing(&env, &market_id, start);

        assert!(!UnclaimedWinningsPolicy::is_claim_window_expired(
            &env,
            &market_id,
            start
        ));
        env.ledger().with_mut(|ledger| ledger.timestamp = start + 59);
        assert!(!UnclaimedWinningsPolicy::is_claim_window_expired(
            &env,
            &market_id,
            start
        ));
        env.ledger().with_mut(|ledger| ledger.timestamp = start + 60);
        assert!(UnclaimedWinningsPolicy::is_claim_window_expired(
            &env,
            &market_id,
            start
        ));
    });
}

#[test]
fn recovery_rejects_an_unauthorized_actor() {
    let env = Env::default();
    let (contract_id, admin) = setup_contract(&env);
    let unauthorized = Address::generate(&env);
    let market_id = Symbol::new(&env, "recovery_auth");

    env.as_contract(&contract_id, || {
        set_admin(&env, &admin);
        let market = make_market(&env, &admin);
        MarketStateManager::update_market(&env, &market_id, &market);

        assert_eq!(
            RecoveryManager::recover_market_state(&env, &unauthorized, &market_id),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn recovery_rejects_a_missing_market() {
    let env = Env::default();
    let (contract_id, admin) = setup_contract(&env);
    let market_id = Symbol::new(&env, "missing_market");

    env.as_contract(&contract_id, || {
        set_admin(&env, &admin);

        assert_eq!(
            RecoveryManager::recover_market_state(&env, &admin, &market_id),
            Err(Error::MarketNotFound)
        );
    });
}