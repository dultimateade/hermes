#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::admin::{AdminInitializer, AdminManager};
use crate::Error;

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    AdminInitializer::initialize(&env, &admin).unwrap();
    (env, admin)
}

#[test]
fn cooldown_active_rejects_admin_action() {
    let (env, admin) = setup();

    // First action starts the cooldown window.
    AdminManager::set_cooldown(&env, 100).unwrap();
    AdminManager::record_admin_action(&env, &admin).unwrap();

    // Advance time but stay inside the cooldown window.
    env.ledger().set_timestamp(env.ledger().timestamp() + 50);

    let result = AdminManager::record_admin_action(&env, &admin);
    assert_eq!(result, Err(Error::CooldownActive));
}

#[test]
fn cooldown_expired_allows_admin_action() {
    let (env, admin) = setup();

    AdminManager::set_cooldown(&env, 100).unwrap();
    AdminManager::record_admin_action(&env, &admin).unwrap();

    // Advance past the cooldown window.
    env.ledger().set_timestamp(env.ledger().timestamp() + 101);

    let result = AdminManager::record_admin_action(&env, &admin);
    assert!(result.is_ok());
}

#[test]
fn cooldown_resets_after_successful_action() {
    let (env, admin) = setup();

    AdminManager::set_cooldown(&env, 100).unwrap();
    AdminManager::record_admin_action(&env, &admin).unwrap();

    // Wait out the first cooldown and perform a second action.
    env.ledger().set_timestamp(env.ledger().timestamp() + 101);
    AdminManager::record_admin_action(&env, &admin).unwrap();

    // Immediately after the second action the cooldown must be active again.
    let result = AdminManager::record_admin_action(&env, &admin);
    assert_eq!(result, Err(Error::CooldownActive));

    // And it must expire relative to the second action, not the first.
    env.ledger().set_timestamp(env.ledger().timestamp() + 101);
    assert!(AdminManager::record_admin_action(&env, &admin).is_ok());
}

#[test]
fn concurrent_admin_calls_during_cooldown_only_one_succeeds() {
    let (env, admin) = setup();

    AdminManager::set_cooldown(&env, 100).unwrap();

    // First call succeeds and opens the cooldown window.
    assert!(AdminManager::record_admin_action(&env, &admin).is_ok());

    // Subsequent calls within the same window are all rejected.
    for _ in 0..3 {
        let result = AdminManager::record_admin_action(&env, &admin);
        assert_eq!(result, Err(Error::CooldownActive));
    }

    // Once the window elapses exactly one more call is permitted.
    env.ledger().set_timestamp(env.ledger().timestamp() + 101);
    assert!(AdminManager::record_admin_action(&env, &admin).is_ok());
    assert_eq!(
        AdminManager::record_admin_action(&env, &admin),
        Err(Error::CooldownActive)
    );
}
