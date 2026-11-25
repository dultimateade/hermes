//! Integration tests for the admin cooldown security feature.
//!
//! Covers the full cooldown lifecycle:
//! - cooldown active: admin action rejected while the window is open
//! - cooldown expired: admin action succeeds once the window elapses
//! - cooldown reset: a successful admin action restarts the timer
//! - concurrent admin calls during cooldown: only the permitted call succeeds

use predictify_hybrid::{PredictifyHybrid, PredictifyHybridClient};
use soroban_sdk::{testutils::Address as _, Address, Env};

const COOLDOWN: u64 = 3_600;

fn setup<'a>() -> (Env, PredictifyHybridClient<'a>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PredictifyHybrid);
    let client = PredictifyHybridClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &COOLDOWN);

    (env, client, admin)
}

#[test]
fn admin_action_rejected_while_cooldown_active() {
    let (env, client, admin) = setup();

    // First admin action starts the cooldown window.
    client.admin_action(&admin);

    // Any further action inside the window must be rejected.
    let result = client.try_admin_action(&admin);
    assert!(result.is_err(), "action during cooldown must be rejected");

    // Still rejected just before the window elapses.
    env.ledger().set_timestamp(COOLDOWN - 1);
    let result = client.try_admin_action(&admin);
    assert!(result.is_err(), "action just before expiry must be rejected");
}

#[test]
fn admin_action_succeeds_after_cooldown_expires() {
    let (env, client, admin) = setup();

    client.admin_action(&admin);

    // Advance past the cooldown window.
    env.ledger().set_timestamp(COOLDOWN + 1);

    let result = client.try_admin_action(&admin);
    assert!(result.is_ok(), "action after cooldown must succeed");
}

#[test]
fn successful_action_resets_cooldown_timer() {
    let (env, client, admin) = setup();

    client.admin_action(&admin);

    // Wait out the first cooldown and perform a second action.
    env.ledger().set_timestamp(COOLDOWN + 1);
    client.admin_action(&admin);

    // The timer must have restarted from the second action.
    let result = client.try_admin_action(&admin);
    assert!(result.is_err(), "cooldown must restart after a successful action");

    // And it must still be active relative to the new start time.
    env.ledger().set_timestamp(COOLDOWN + COOLDOWN - 1);
    let result = client.try_admin_action(&admin);
    assert!(result.is_err(), "cooldown must be measured from the latest action");

    // Only after the full window from the second action does it clear.
    env.ledger().set_timestamp(COOLDOWN + COOLDOWN + 1);
    let result = client.try_admin_action(&admin);
    assert!(result.is_ok(), "action after reset cooldown must succeed");
}

#[test]
fn concurrent_admin_calls_during_cooldown_only_permitted_call_succeeds() {
    let (env, client, admin) = setup();

    // First action opens the cooldown window.
    client.admin_action(&admin);

    // Simulate several concurrent admin calls within the same ledger time.
    let mut successes = 0u32;
    for _ in 0..5 {
        if client.try_admin_action(&admin).is_ok() {
            successes += 1;
        }
    }

    assert_eq!(successes, 0, "no concurrent call may bypass an active cooldown");

    // Once the window elapses, exactly one of the concurrent calls succeeds
    // and re-arms the cooldown for the rest.
    env.ledger().set_timestamp(COOLDOWN + 1);

    let mut successes = 0u32;
    for _ in 0..5 {
        if client.try_admin_action(&admin).is_ok() {
            successes += 1;
        }
    }

    assert_eq!(successes, 1, "only the first permitted call may succeed");
}
