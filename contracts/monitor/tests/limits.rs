//! # Limits feature tests for the Monitor contract
//!
//! Covers:
//! - Default cap values after initialization
//! - Admin-configurable cap updates (valid, invalid, boundary)
//! - record_*/remove_* happy paths with count assertions
//! - Cap-exceeded errors at the boundary (off-by-one)
//! - Underflow guard on remove when count is zero
//! - Isolation between distinct accounts
//! - get_account_state and get_caps read paths
//! - Pre-initialization guard for state-changing entrypoints
//! - Cap increase / decrease behavioural semantics
//! - version() constant

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};

use monitor::{
    AccountState, CapType, Caps, MonitorContract, MonitorContractClient, MonitorError,
};

// ---------------------------------------------------------------------------
// Test helpers
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

struct TestSetup {
    admin: Address,
    user1: Address,
    user2: Address,
    client: MonitorContractClient,
}

/// Create an initialized Monitor contract and return `(env, setup)`.
///
/// Both must be kept alive for the duration of the test because the client
/// holds a borrow of `env`.
fn setup() -> (Env, TestSetup) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    let setup = TestSetup {
        admin,
        user1,
        user2,
        client,
    };

    setup.client.initialize(&setup.admin);

    (env, setup)
}

// ---------------------------------------------------------------------------
// §1 Initialization
// ---------------------------------------------------------------------------

#[test]
fn test_initialize_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert!(client.try_initialize(&admin).is_ok());
}

#[test]
fn test_initialize_twice_returns_already_initialized() {
    let (_env, setup) = setup();
    let result = setup.client.try_initialize(&setup.admin);
    assert_eq!(result, Ok(Err(MonitorError::AlreadyInitialized)));
}

// ---------------------------------------------------------------------------
// §2 Default caps
// ---------------------------------------------------------------------------

#[test]
fn test_default_caps() {
    let (_env, setup) = setup();
    let caps: Caps = setup.client.get_caps();
    assert_eq!(caps.max_bets, 50);
    assert_eq!(caps.max_positions, 20);
    assert_eq!(caps.max_subscriptions, 10);
}

// ---------------------------------------------------------------------------
// §3 set_caps — valid paths
// ---------------------------------------------------------------------------

#[test]
fn test_set_bets_cap() {
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Bets, &5u32);
    let caps = setup.client.get_caps();
    assert_eq!(caps.max_bets, 5);
    assert_eq!(caps.max_positions, 20);
    assert_eq!(caps.max_subscriptions, 10);
}

#[test]
fn test_set_positions_cap() {
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Positions, &3u32);
    assert_eq!(setup.client.get_caps().max_positions, 3);
}

#[test]
fn test_set_subscriptions_cap() {
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Subscriptions, &2u32);
    assert_eq!(setup.client.get_caps().max_subscriptions, 2);
}

#[test]
fn test_set_caps_exactly_hard_upper_bound_succeeds() {
    let (_env, setup) = setup();
    let result = setup
        .client
        .try_set_caps(&setup.admin, &CapType::Bets, &10_000u32);
    assert!(result.is_ok());
    assert_eq!(setup.client.get_caps().max_bets, 10_000);
}

// ---------------------------------------------------------------------------
// §4 set_caps — invalid inputs
// ---------------------------------------------------------------------------

#[test]
fn test_set_caps_zero_returns_invalid_input() {
    let (_env, setup) = setup();
    let result = setup
        .client
        .try_set_caps(&setup.admin, &CapType::Bets, &0u32);
    assert_eq!(result, Ok(Err(MonitorError::InvalidInput)));
}

#[test]
fn test_set_caps_exceeds_hard_upper_bound_returns_invalid_input() {
    let (_env, setup) = setup();
    let result = setup
        .client
        .try_set_caps(&setup.admin, &CapType::Bets, &10_001u32);
    assert_eq!(result, Ok(Err(MonitorError::InvalidInput)));
}

#[test]
fn test_set_caps_non_admin_returns_unauthorized() {
    let (_env, setup) = setup();
    let result = setup
        .client
        .try_set_caps(&setup.user1, &CapType::Bets, &5u32);
    assert_eq!(result, Ok(Err(MonitorError::Unauthorized)));
}

// ---------------------------------------------------------------------------
// §5 record_bet / remove_bet — happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_record_bet_increments_to_one() {
    let (_env, setup) = setup();
    let count = setup.client.record_bet(&setup.user1);
    assert_eq!(count, 1);
    assert_eq!(setup.client.get_account_state(&setup.user1).unwrap().bets, 1);
}

#[test]
fn test_record_bet_twice() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    let count = setup.client.record_bet(&setup.user1);
    assert_eq!(count, 2);
}

#[test]
fn test_record_bet_up_to_cap() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Bets, &3u32);
    setup.client.record_bet(&setup.user1);
    setup.client.record_bet(&setup.user1);
    let count = setup.client.record_bet(&setup.user1);
    assert_eq!(count, 3);
}

#[test]
fn test_remove_bet_decrements_count() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    setup.client.record_bet(&setup.user1);
    let count = setup.client.remove_bet(&setup.user1);
    assert_eq!(count, 1);
}

#[test]
fn test_remove_bet_to_zero() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    let count = setup.client.remove_bet(&setup.user1);
    assert_eq!(count, 0);
}

#[test]
fn test_record_bet_after_remove_succeeds() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Bets, &1u32);
    setup.client.record_bet(&setup.user1);
    setup.client.remove_bet(&setup.user1);
    let count = setup.client.record_bet(&setup.user1);
    assert_eq!(count, 1);
}

// ---------------------------------------------------------------------------
// §6 record_bet / remove_bet — error paths
// ---------------------------------------------------------------------------

#[test]
fn test_record_bets_exactly_at_cap_then_rejects_next() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Bets, &3u32);
    let caps = setup.client.get_caps();

    for expected_count in 1..=caps.max_bets {
        assert_eq!(
            setup.client.record_bet(&setup.user1),
            expected_count,
            "recording bet {expected_count} should succeed up to max_bets"
        );
    }
    assert_eq!(
        setup.client.get_account_state(&setup.user1).unwrap().bets,
        caps.max_bets
    );

    let result = setup.client.try_record_bet(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::BetCapExceeded)));
}

#[test]
fn test_remove_bet_underflow_returns_error() {
    let (_env, setup) = setup();
    let result = setup.client.try_remove_bet(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::Underflow)));
}

// ---------------------------------------------------------------------------
// §7 record_position / remove_position
// ---------------------------------------------------------------------------

#[test]
fn test_record_position_increments_count() {
    let (_env, setup) = setup();
    let count = setup.client.record_position(&setup.user1);
    assert_eq!(count, 1);
}

#[test]
fn test_record_position_up_to_cap() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Positions, &2u32);
    setup.client.record_position(&setup.user1);
    let count = setup.client.record_position(&setup.user1);
    assert_eq!(count, 2);
}

#[test]
fn test_record_position_exceeds_cap_returns_error() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Positions, &1u32);
    setup.client.record_position(&setup.user1);
    let result = setup.client.try_record_position(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::PositionCapExceeded)));
}

#[test]
fn test_remove_position_decrements_count() {
    let (_env, setup) = setup();
    setup.client.record_position(&setup.user1);
    let count = setup.client.remove_position(&setup.user1);
    assert_eq!(count, 0);
}

#[test]
fn test_remove_position_underflow_returns_error() {
    let (_env, setup) = setup();
    let result = setup.client.try_remove_position(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::Underflow)));
}

// ---------------------------------------------------------------------------
// §8 record_subscription / remove_subscription
// ---------------------------------------------------------------------------

#[test]
fn test_record_subscription_increments_count() {
    let (_env, setup) = setup();
    let count = setup.client.record_subscription(&setup.user1);
    assert_eq!(count, 1);
}

#[test]
fn test_record_subscription_up_to_cap() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &3u32);
    setup.client.record_subscription(&setup.user1);
    setup.client.record_subscription(&setup.user1);
    let count = setup.client.record_subscription(&setup.user1);
    assert_eq!(count, 3);
}

#[test]
fn test_record_subscription_exceeds_cap_returns_error() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &1u32);
    setup.client.record_subscription(&setup.user1);
    let result = setup.client.try_record_subscription(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::SubscriptionCapExceeded)));
}

#[test]
fn test_remove_subscription_decrements_count() {
    let (_env, setup) = setup();
    setup.client.record_subscription(&setup.user1);
    let count = setup.client.remove_subscription(&setup.user1);
    assert_eq!(count, 0);
}

#[test]
fn test_remove_subscription_underflow_returns_error() {
    let (_env, setup) = setup();
    let result = setup.client.try_remove_subscription(&setup.user1);
    assert_eq!(result, Ok(Err(MonitorError::Underflow)));
}

// ---------------------------------------------------------------------------
// §9 Account isolation
// ---------------------------------------------------------------------------

#[test]
fn test_accounts_are_isolated() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Bets, &1u32);

    setup.client.record_bet(&setup.user1);
    // user1 is at cap
    assert_eq!(
        setup.client.try_record_bet(&setup.user1),
        Ok(Err(MonitorError::BetCapExceeded))
    );
    // user2 is unaffected
    let count = setup.client.record_bet(&setup.user2);
    assert_eq!(count, 1);
}

#[test]
fn test_counts_are_per_account() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    setup.client.record_bet(&setup.user1);
    setup.client.record_bet(&setup.user2);

    assert_eq!(setup.client.get_account_state(&setup.user1).unwrap().bets, 2);
    assert_eq!(setup.client.get_account_state(&setup.user2).unwrap().bets, 1);
}

// ---------------------------------------------------------------------------
// §10 get_account_state
// ---------------------------------------------------------------------------

#[test]
fn test_get_account_state_returns_none_for_new_user() {
    let (_env, setup) = setup();
    assert_eq!(setup.client.get_account_state(&setup.user1), None);
}

#[test]
fn test_get_account_state_returns_some_after_first_record_bet() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    assert!(setup.client.get_account_state(&setup.user1).is_some());
}

#[test]
fn test_get_account_state_returns_some_after_first_record_position() {
    let (_env, setup) = setup();
    setup.client.record_position(&setup.user1);
    assert!(setup.client.get_account_state(&setup.user1).is_some());
}

#[test]
fn test_get_account_state_returns_some_after_first_record_subscription() {
    let (_env, setup) = setup();
    setup.client.record_subscription(&setup.user1);
    assert!(setup.client.get_account_state(&setup.user1).is_some());
}

#[test]
fn test_get_account_state_remains_some_after_all_decremented_to_zero() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    setup.client.remove_bet(&setup.user1);
    // Account is tracked even though every counter is back at zero.
    assert_eq!(
        setup.client.get_account_state(&setup.user1),
        Some(AccountState {
            bets: 0,
            positions: 0,
            subscriptions: 0,
        })
    );
}

#[test]
fn test_get_account_state_reflects_all_resource_types() {
    let (_env, setup) = setup();
    setup.client.record_bet(&setup.user1);
    setup.client.record_bet(&setup.user1);
    setup.client.record_position(&setup.user1);
    setup.client.record_subscription(&setup.user1);
    setup.client.record_subscription(&setup.user1);
    setup.client.record_subscription(&setup.user1);

    let state = setup.client.get_account_state(&setup.user1).unwrap();
    assert_eq!(state.bets, 2);
    assert_eq!(state.positions, 1);
    assert_eq!(state.subscriptions, 3);
}

// ---------------------------------------------------------------------------
// §11 Pre-initialization guard
// ---------------------------------------------------------------------------

#[test]
fn test_record_bet_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_record_bet(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_record_position_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_record_position(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_record_subscription_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_record_subscription(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_remove_bet_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_remove_bet(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_remove_position_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_remove_position(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_remove_subscription_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_remove_subscription(&user),
        Ok(Err(MonitorError::NotInitialized))
    );
}

#[test]
fn test_set_caps_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, MonitorContract);
    let client = MonitorContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_set_caps(&admin, &CapType::Bets, &5u32),
        Ok(Err(MonitorError::NotInitialized))
    );
}

// ---------------------------------------------------------------------------
// §12 version
// ---------------------------------------------------------------------------

#[test]
fn test_version_returns_one() {
    let (_env, setup) = setup();
    assert_eq!(setup.client.version(), 1u32);
}

// ---------------------------------------------------------------------------
// §13 Cap boundary — exactly at cap is rejected (off-by-one guard)
// ---------------------------------------------------------------------------

#[test]
fn test_bet_cap_boundary_exact() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Bets, &3u32);
    for _ in 0..3 {
        setup.client.record_bet(&setup.user1);
    }
    assert_eq!(
        setup.client.try_record_bet(&setup.user1),
        Ok(Err(MonitorError::BetCapExceeded))
    );
}

#[test]
fn test_position_cap_boundary_exact() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Positions, &2u32);
    setup.client.record_position(&setup.user1);
    setup.client.record_position(&setup.user1);
    assert_eq!(
        setup.client.try_record_position(&setup.user1),
        Ok(Err(MonitorError::PositionCapExceeded))
    );
}

#[test]
fn test_subscription_cap_boundary_exact() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &1u32);
    setup.client.record_subscription(&setup.user1);
    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );
}

// ---------------------------------------------------------------------------
// §14 Remove then re-record stays within cap
// ---------------------------------------------------------------------------

#[test]
fn test_position_cap_enforcement_after_remove_and_re_record() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Positions, &2u32);
    setup.client.record_position(&setup.user1);
    setup.client.record_position(&setup.user1);

    assert_eq!(
        setup.client.try_record_position(&setup.user1),
        Ok(Err(MonitorError::PositionCapExceeded))
    );

    setup.client.remove_position(&setup.user1);

    let count = setup.client.record_position(&setup.user1);
    assert_eq!(count, 2);
}

// ---------------------------------------------------------------------------
// §15 Cap increase allows previously-capped accounts to record more
// ---------------------------------------------------------------------------

#[test]
fn test_cap_increase_allows_new_records() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &1u32);
    setup.client.record_subscription(&setup.user1);

    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );

    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &3u32);

    let count = setup.client.record_subscription(&setup.user1);
    assert_eq!(count, 2);
}

// ---------------------------------------------------------------------------
// §16 Cap decrease rejects new records even if current count is below old cap
// ---------------------------------------------------------------------------

#[test]
fn test_cap_decrease_blocks_new_records() {
    let (_env, setup) = setup();
    setup.client.record_subscription(&setup.user1);

    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &1u32);

    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );
}

// ---------------------------------------------------------------------------
// §17 Mixed resource types don't interfere
// ---------------------------------------------------------------------------

#[test]
fn test_different_resource_types_are_independent() {
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &1u32);
    setup.client.record_subscription(&setup.user1);
    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );

    let bet_count = setup.client.record_bet(&setup.user1);
    assert_eq!(bet_count, 1);
    let pos_count = setup.client.record_position(&setup.user1);
    assert_eq!(pos_count, 1);
}

// ---------------------------------------------------------------------------
// §18 Per-type cap boundary: cap−1 (succeeds), cap (succeeds then blocked),
//     cap+1 attempt (blocked a second time).
//
// For each CapType the fixture sets the cap to N=5, then:
//   - fills to N−1  → last record call succeeds and returns N−1
//   - records once more (count becomes N)  → succeeds and returns N
//   - tries to record again (would be N+1) → CapExceeded error
//
// A second try_record confirms the error is stable (not a one-shot latch).
// ---------------------------------------------------------------------------

// --- Bets ---

/// Record N−1 bets: each call must succeed, final count must equal N−1.
#[test]
fn test_bets_cap_minus_one_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Bets, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_bet(&setup.user1);
    }

    let state = setup.client.get_account_state(&setup.user1).unwrap();
    assert_eq!(state.bets, CAP - 1);
}

/// Record exactly N bets (the cap): the N-th call must succeed and return N.
#[test]
fn test_bets_at_cap_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Bets, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_bet(&setup.user1);
    }
    let count = setup.client.record_bet(&setup.user1);

    assert_eq!(count, CAP);
}

/// Attempt to record a bet beyond the cap (N+1): must return BetCapExceeded,
/// and a second attempt must return the same error (not a one-shot latch).
#[test]
fn test_bets_cap_plus_one_rejected() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Bets, &CAP);

    for _ in 0..CAP {
        setup.client.record_bet(&setup.user1);
    }

    assert_eq!(
        setup.client.try_record_bet(&setup.user1),
        Ok(Err(MonitorError::BetCapExceeded))
    );
    // Second attempt: error must be stable, not a one-shot latch.
    assert_eq!(
        setup.client.try_record_bet(&setup.user1),
        Ok(Err(MonitorError::BetCapExceeded))
    );
}

// --- Positions ---

/// Record N−1 positions: each call must succeed, final count must equal N−1.
#[test]
fn test_positions_cap_minus_one_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Positions, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_position(&setup.user1);
    }

    let state = setup.client.get_account_state(&setup.user1).unwrap();
    assert_eq!(state.positions, CAP - 1);
}

/// Record exactly N positions (the cap): the N-th call must succeed and return N.
#[test]
fn test_positions_at_cap_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Positions, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_position(&setup.user1);
    }
    let count = setup.client.record_position(&setup.user1);

    assert_eq!(count, CAP);
}

/// Attempt to record a position beyond the cap (N+1): must return
/// PositionCapExceeded, and a second attempt must return the same error.
#[test]
fn test_positions_cap_plus_one_rejected() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup.client.set_caps(&setup.admin, &CapType::Positions, &CAP);

    for _ in 0..CAP {
        setup.client.record_position(&setup.user1);
    }

    assert_eq!(
        setup.client.try_record_position(&setup.user1),
        Ok(Err(MonitorError::PositionCapExceeded))
    );
    assert_eq!(
        setup.client.try_record_position(&setup.user1),
        Ok(Err(MonitorError::PositionCapExceeded))
    );
}

// --- Subscriptions ---

/// Record N−1 subscriptions: each call must succeed, final count must equal N−1.
#[test]
fn test_subscriptions_cap_minus_one_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_subscription(&setup.user1);
    }

    let state = setup.client.get_account_state(&setup.user1).unwrap();
    assert_eq!(state.subscriptions, CAP - 1);
}

/// Record exactly N subscriptions (the cap): the N-th call must succeed and
/// return N.
#[test]
fn test_subscriptions_at_cap_succeeds() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &CAP);

    for _ in 0..CAP - 1 {
        setup.client.record_subscription(&setup.user1);
    }
    let count = setup.client.record_subscription(&setup.user1);

    assert_eq!(count, CAP);
}

/// Attempt to record a subscription beyond the cap (N+1): must return
/// SubscriptionCapExceeded, and a second attempt must return the same error.
#[test]
fn test_subscriptions_cap_plus_one_rejected() {
    const CAP: u32 = 5;
    let (_env, setup) = setup();
    setup
        .client
        .set_caps(&setup.admin, &CapType::Subscriptions, &CAP);

    for _ in 0..CAP {
        setup.client.record_subscription(&setup.user1);
    }

    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );
    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );
}

// ---------------------------------------------------------------------------
// §19 Per-type cap boundary at default cap values
//
// Drives each resource type all the way to its compile-time default cap
// without calling set_caps, so a bug in the fallback `.unwrap_or(DEFAULT_*)` 
// path or in the default constants themselves is caught here.
// ---------------------------------------------------------------------------

/// Drive bets to the default cap (50): the 50th record must succeed; the 51st
/// must return BetCapExceeded.
#[test]
fn test_bets_default_cap_boundary() {
    let (_env, setup) = setup();
    // Default cap is DEFAULT_MAX_BETS = 50; no set_caps call.
    for _ in 0..49 {
        setup.client.record_bet(&setup.user1);
    }
    // 50th — must succeed (count == cap).
    let count = setup.client.record_bet(&setup.user1);
    assert_eq!(count, 50);

    // 51st — must fail.
    assert_eq!(
        setup.client.try_record_bet(&setup.user1),
        Ok(Err(MonitorError::BetCapExceeded))
    );
}

/// Drive positions to the default cap (20): the 20th record must succeed; the
/// 21st must return PositionCapExceeded.
#[test]
fn test_positions_default_cap_boundary() {
    let (_env, setup) = setup();
    // Default cap is DEFAULT_MAX_POSITIONS = 20; no set_caps call.
    for _ in 0..19 {
        setup.client.record_position(&setup.user1);
    }
    // 20th — must succeed (count == cap).
    let count = setup.client.record_position(&setup.user1);
    assert_eq!(count, 20);

    // 21st — must fail.
    assert_eq!(
        setup.client.try_record_position(&setup.user1),
        Ok(Err(MonitorError::PositionCapExceeded))
    );
}

/// Drive subscriptions to the default cap (10): the 10th record must succeed;
/// the 11th must return SubscriptionCapExceeded.
#[test]
fn test_subscriptions_default_cap_boundary() {
    let (_env, setup) = setup();
    // Default cap is DEFAULT_MAX_SUBSCRIPTIONS = 10; no set_caps call.
    for _ in 0..9 {
        setup.client.record_subscription(&setup.user1);
    }
    // 10th — must succeed (count == cap).
    let count = setup.client.record_subscription(&setup.user1);
    assert_eq!(count, 10);

    // 11th — must fail.
    assert_eq!(
        setup.client.try_record_subscription(&setup.user1),
        Ok(Err(MonitorError::SubscriptionCapExceeded))
    );
}
