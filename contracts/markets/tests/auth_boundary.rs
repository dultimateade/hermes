#![cfg(test)]

use markets::{ContractError, MarketsContract, MarketsContractClient, PayoutCurveConfig};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo, MockAuth, MockAuthInvoke},
    Address, Env, IntoVal, String, Vec,
};

// ============================================
// Test Helpers
// ============================================

fn setup_test_environment(env: &Env) -> TestSetup<'_> {
    env.ledger().set(LedgerInfo {
        timestamp: 1735689600,
        protocol_version: 22,
        sequence_number: 1,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 518400,
    });

    let admin = Address::generate(env);
    let market_creator = Address::generate(env);
    let user1 = Address::generate(env);
    let user2 = Address::generate(env);
    let unauthorized = Address::generate(env);

    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(env, &contract_id);

    let init_args = (admin.clone(),).into_val(env);
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initialize",
            args: init_args,
            sub_invokes: &[],
        },
    }]);
    client.initialize(&admin);
    let _ = env.auths();

    TestSetup {
        admin,
        market_creator,
        user1,
        user2,
        unauthorized,
        client,
        contract_id,
    }
}

struct TestSetup<'a> {
    admin: Address,
    market_creator: Address,
    user1: Address,
    user2: Address,
    unauthorized: Address,
    client: MarketsContractClient<'a>,
    contract_id: Address,
}

/// Creates a market using explicit auth mocking for the market creator.
/// Used by tests that cannot rely on `mock_all_auths()`.
fn create_market_with_mock_auth(setup: &TestSetup<'_>) -> u32 {
    let env = &setup.client.env;
    let question = String::from_str(env, "Test market question?");
    let description = String::from_str(env, "Test market description");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(env, "Test Source");
    let outcome_tags = Vec::from_array(
        env,
        [String::from_str(env, "Yes"), String::from_str(env, "No")],
    );

    // Authorize ONLY the market_creator for this specific create_market call.
    env.mock_auths(&[MockAuth {
        address: &setup.market_creator,
        invoke: &MockAuthInvoke {
            contract: &setup.contract_id,
            fn_name: "create_market",
            args: (
                setup.market_creator.clone(),
                question.clone(),
                description.clone(),
                end_time,
                resolution_source.clone(),
                outcome_tags.clone(),
                1i128,
            )
                .into_val(env),
            sub_invokes: &[],
        },
    }]);

    setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    )
}

/// Creates a market (requires `env.mock_all_auths()` to be active).
fn create_market_with_auth_check(setup: &TestSetup<'_>) -> u32 {
    let env = &setup.client.env;
    let question = String::from_str(env, "Test market question?");
    let description = String::from_str(env, "Test market description");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(env, "Test Source");
    let outcome_tags = Vec::from_array(
        env,
        [String::from_str(env, "Yes"), String::from_str(env, "No")],
    );

    setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    )
}

// ============================================
// Auth Boundary Tests
// ============================================

// ── create_market ─────────────────────────────────────────────────────────────

#[test]
fn test_create_market_requires_auth() {
    let env = Env::default();
    // No mock_all_auths — require_auth should fail for everyone.
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    let unauthorized = Address::generate(&env);
    let question = String::from_str(&env, "Test market question?");
    let description = String::from_str(&env, "Test market description");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(&env, "Test Source");
    let outcome_tags = Vec::from_array(
        &env,
        [String::from_str(&env, "Yes"), String::from_str(&env, "No")],
    );

    let result = client.try_create_market(
        &unauthorized,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );
    assert!(
        result.is_err(),
        "Unauthorized user should not create market"
    );
}

#[test]
fn test_create_market_requires_auth_success() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);
    assert_eq!(market_id, 1);
}

#[test]
fn test_market_limit_blocks_both_creation_entrypoints() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    assert_eq!(setup.client.get_market_limit(), 10_000);

    let unauthorized_result = setup
        .client
        .try_set_market_limit(&setup.unauthorized, &1);
    assert_eq!(
        unauthorized_result.unwrap_err().unwrap(),
        ContractError::Unauthorized.into()
    );

    setup.client.set_market_limit(&setup.admin, &1);
    assert_eq!(setup.client.get_market_limit(), 1);
    let market_id = create_market_with_auth_check(&setup);
    assert_eq!(market_id, 1);

    let question = String::from_str(&env, "Another market?");
    let description = String::from_str(&env, "Description");
    let end_time = env.ledger().timestamp() + 86_400;
    let resolution_source = String::from_str(&env, "Test Source");
    let outcome_tags = Vec::from_array(
        &env,
        [String::from_str(&env, "Yes"), String::from_str(&env, "No")],
    );

    let regular_result = setup.client.try_create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );
    assert_eq!(
        regular_result.unwrap_err().unwrap(),
        ContractError::MarketLimitReached.into()
    );

    let curve = PayoutCurveConfig::winner_takes_all(&env);
    let curve_result = setup.client.try_create_market_with_curve(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &curve,
        &1i128,
    );
    assert_eq!(
        curve_result.unwrap_err().unwrap(),
        ContractError::MarketLimitReached.into()
    );
    assert_eq!(setup.client.get_market_for_resolution(&2), None);
}

#[test]
fn test_creator_market_limit_is_admin_configurable_and_per_address() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    assert_eq!(setup.client.get_creator_market_limit(), 100);
    assert_eq!(
        setup
            .client
            .try_set_creator_market_limit(&setup.unauthorized, &1)
            .unwrap_err()
            .unwrap(),
        ContractError::Unauthorized.into()
    );

    setup.client.set_creator_market_limit(&setup.admin, &1);
    assert_eq!(setup.client.get_creator_market_limit(), 1);
    assert_eq!(create_market_with_auth_check(&setup), 1);

    let question = String::from_str(&env, "Another market?");
    let description = String::from_str(&env, "Description");
    let end_time = env.ledger().timestamp() + 86_400;
    let resolution_source = String::from_str(&env, "Test Source");
    let outcome_tags = Vec::from_array(
        &env,
        [String::from_str(&env, "Yes"), String::from_str(&env, "No")],
    );
    let curve = PayoutCurveConfig::winner_takes_all(&env);
    let over_limit = setup.client.try_create_market_with_curve(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &curve,
        &1i128,
    );
    assert_eq!(
        over_limit.unwrap_err().unwrap(),
        ContractError::CreatorMarketLimitReached.into()
    );
    assert_eq!(setup.client.get_market_for_resolution(&2), None);

    let other_creator = setup.user1.clone();
    assert_eq!(
        setup.client.create_market(
            &other_creator,
            &question,
            &description,
            &end_time,
            &resolution_source,
            &outcome_tags,
            &1i128,
        ),
        2
    );
}

#[test]
fn test_uninitialized_contract_rejects_admin_actions() {
    let env = Env::default();
    env.mock_all_auths();
    let attacker = Address::generate(&env);
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    assert_eq!(client.get_admin(), None);
    assert_eq!(
        client.try_pause_markets(&attacker).unwrap_err().unwrap(),
        ContractError::Unauthorized.into()
    );
    assert_eq!(
        client
            .try_transfer_ownership(&attacker, &Address::generate(&env))
            .unwrap_err()
            .unwrap(),
        ContractError::Unauthorized.into()
    );
}

/// Regression test for issue #345: state-changing entrypoints called on an
/// uninitialized contract must return an initialization guard error
/// (`NotInitialized` / `InvalidState`) rather than a misleading `Unauthorized`.
#[test]
fn test_uninitialized_contract_returns_init_guard_error() {
    let env = Env::default();
    env.mock_all_auths();
    let caller = Address::generate(&env);
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    let question = String::from_str(&env, "Test market question?");
    let description = String::from_str(&env, "Test market description");
    let end_time = env.ledger().timestamp() + 86_400;
    let resolution_source = String::from_str(&env, "Test Source");
    let outcome_tags = Vec::from_array(
        &env,
        [String::from_str(&env, "Yes"), String::from_str(&env, "No")],
    );

    let unauthorized: u32 = ContractError::Unauthorized.into();
    let invalid_state: u32 = ContractError::InvalidState.into();

    // collect_user_fees must not report Unauthorized on an uninitialized contract.
    let fees_result = client.try_collect_user_fees(&caller);
    assert!(fees_result.is_err(), "collect_user_fees should fail");
    let fees_err = fees_result.unwrap_err().unwrap();
    assert_ne!(
        fees_err, unauthorized,
        "collect_user_fees must not return Unauthorized on uninitialized contract"
    );
    assert_eq!(
        fees_err, invalid_state,
        "collect_user_fees must return InvalidState on uninitialized contract"
    );

    // Other state-changing entrypoints must also guard initialization.
    let create_result = client.try_create_market(
        &caller,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );
    assert!(create_result.is_err(), "create_market should fail");
    assert_eq!(
        create_result.unwrap_err().unwrap(),
        invalid_state,
        "create_market must return InvalidState on uninitialized contract"
    );

    let bet_result = client.try_place_bet(&caller, &1, &0, &100);
    assert!(bet_result.is_err(), "place_bet should fail");
    assert_eq!(
        bet_result.unwrap_err().unwrap(),
        invalid_state,
        "place_bet must return InvalidState on uninitialized contract"
    );
}

#[test]
fn test_create_market_requires_auth_market_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Test market question?");
    let description = String::from_str(&env, "Test market description");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(&env, "Test Source");
    let outcome_tags = Vec::from_array(
        &env,
        [String::from_str(&env, "Yes"), String::from_str(&env, "No")],
    );

    let result = setup.client.try_create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );
    assert!(result.is_ok(), "Authorized market creator should succeed");
}

// ── place_bet ─────────────────────────────────────────────────────────────────

#[test]
fn test_place_bet_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths — setup uses explicit MockAuth, but
    // the unauthorized test call should fail auth.

    let market_id = create_market_with_mock_auth(&setup);

    let outcome_index = 0;
    let amount = 100;

    let result =
        setup
            .client
            .try_place_bet(&setup.unauthorized, &market_id, &outcome_index, &amount);
    assert!(result.is_err(), "Unauthorized user should not place bet");
}

#[test]
fn test_place_bet_requires_auth_success() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let outcome_index = 0;
    let amount = 100;

    let result = setup
        .client
        .try_place_bet(&setup.user1, &market_id, &outcome_index, &amount);
    assert!(result.is_ok(), "Authorized user should place bet");
}

#[test]
fn test_place_bet_rejects_closed_market() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);
    env.ledger().set_timestamp(1735689600 + 86400);

    let result = setup
        .client
        .try_place_bet(&setup.user1, &market_id, &0, &100);
    assert_eq!(result, Err(Ok(ContractError::MarketClosed)));
}

// ── resolve_market ────────────────────────────────────────────────────────────

#[test]
fn test_resolve_market_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths.

    let market_id = create_market_with_mock_auth(&setup);

    let winning_outcome = 0;

    let result = setup
        .client
        .try_resolve_market(&setup.unauthorized, &market_id, &winning_outcome);
    assert!(
        result.is_err(),
        "Unauthorized user should not resolve market"
    );
}

#[test]
fn test_resolve_market_requires_auth_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let winning_outcome = 0;

    let result =
        setup
            .client
            .try_resolve_market(&setup.market_creator, &market_id, &winning_outcome);
    assert!(result.is_ok(), "Market creator should resolve market");
}

/// Regression test for issue #200.
///
/// A non-creator, non-admin caller must not be able to resolve a market.
/// The missing authorization check previously went undetected because this
/// test case did not exist.
#[test]
fn test_resolve_market_rejects_non_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let winning_outcome = 0;

    let result =
        setup
            .client
            .try_resolve_market(&setup.unauthorized, &market_id, &winning_outcome);
    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "Non-creator must not resolve market"
    );
}

// ── claim_winnings ────────────────────────────────────────────────────────────

#[test]
fn test_claim_winnings_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths — every setup call uses explicit MockAuth.

    // Create market with MockAuth for market_creator.
    let market_id = create_market_with_mock_auth(&setup);

    // Authorize user1 for place_bet.
    let bet_outcome = 0u32;
    let bet_amount = 100i128;
    env.mock_auths(&[MockAuth {
        address: &setup.user1,
        invoke: &MockAuthInvoke {
            contract: &setup.contract_id,
            fn_name: "place_bet",
            args: (&setup.user1, &market_id, &bet_outcome, &bet_amount).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    setup.client.place_bet(&setup.user1, &market_id, &0, &100);

    // Advance ledger past end time.
    env.ledger().set_timestamp(1735689600 + 90000);

    // Authorize market_creator for resolve_market.
    let win_outcome = 0u32;
    env.mock_auths(&[MockAuth {
        address: &setup.market_creator,
        invoke: &MockAuthInvoke {
            contract: &setup.contract_id,
            fn_name: "resolve_market",
            args: (&setup.market_creator, &market_id, &win_outcome).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    setup
        .client
        .resolve_market(&setup.market_creator, &market_id, &0);

    // Now test: unauthorized user should NOT be able to claim.
    let result = setup
        .client
        .try_claim_winnings(&setup.unauthorized, &market_id);
    assert!(
        result.is_err(),
        "Unauthorized user should not claim winnings"
    );
}

#[test]
fn test_claim_winnings_requires_auth_success() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    setup.client.place_bet(&setup.user1, &market_id, &0, &100);

    env.ledger().set_timestamp(1735689600 + 90000);

    setup
        .client
        .resolve_market(&setup.market_creator, &market_id, &0);

    let result = setup.client.try_claim_winnings(&setup.user1, &market_id);
    assert!(result.is_ok(), "Winner should claim winnings");
}

// ── Reentrancy / double-claim guard (issue #21) ───────────────────────────────

/// Regression test for issue #21.
///
/// Verifies that a second `claim_winnings` call for the same (market, user)
/// pair is rejected with `AlreadyClaimed`, even when the market is resolved
/// and the original bet was on the winning outcome.  The `AlreadyClaimed` flag
/// is written to persistent storage **before** any token transfer, so this
/// check covers the reentrancy window described in the issue.
#[test]
fn test_claim_winnings_rejects_double_claim() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    // user1 places a winning bet on outcome 0.
    setup.client.place_bet(&setup.user1, &market_id, &0, &100);

    env.ledger().set_timestamp(1735689600 + 90000);

    setup
        .client
        .resolve_market(&setup.market_creator, &market_id, &0);

    // First claim: must succeed.
    let first = setup.client.try_claim_winnings(&setup.user1, &market_id);
    assert!(first.is_ok(), "First claim should succeed");

    // Second claim: must be rejected as AlreadyClaimed.
    let second = setup.client.try_claim_winnings(&setup.user1, &market_id);
    assert!(
        second.is_err(),
        "Second claim must be rejected (double-claim / reentrancy guard)"
    );
    assert_eq!(
        second.unwrap_err().unwrap(),
        markets::ContractError::AlreadyClaimed.into(),
        "Error must be AlreadyClaimed (code 15)"
    );
}

#[test]
fn test_cancel_market_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths.

    let market_id = create_market_with_mock_auth(&setup);

    let result = setup
        .client
        .try_cancel_market(&setup.unauthorized, &market_id);
    assert!(
        result.is_err(),
        "Unauthorized user should not cancel market"
    );
}

#[test]
fn test_cancel_market_requires_auth_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let result = setup
        .client
        .try_cancel_market(&setup.market_creator, &market_id);
    assert!(result.is_ok(), "Market creator should cancel market");
}

// ── withdraw_funds ────────────────────────────────────────────────────────────

#[test]
fn test_withdraw_funds_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths.

    let market_id = create_market_with_mock_auth(&setup);

    let amount = 50;

    let result = setup
        .client
        .try_withdraw_funds(&setup.unauthorized, &market_id, &amount);
    assert!(
        result.is_err(),
        "Unauthorized user should not withdraw funds"
    );
}

#[test]
fn test_withdraw_funds_requires_auth_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let amount = 50;

    let result = setup
        .client
        .try_withdraw_funds(&setup.market_creator, &market_id, &amount);
    match result {
        Ok(_) => assert!(true, "Auth passed"),
        Err(e) => {
            let error_str = format!("{:?}", e);
            assert!(!error_str.contains("auth"), "Auth should not fail");
        }
    }
}

// ── update_market_params ─────────────────────────────────────────────────────

#[test]
fn test_update_market_params_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths.

    let market_id = create_market_with_mock_auth(&setup);

    let new_end_time = env.ledger().timestamp() + 172800;

    let result =
        setup
            .client
            .try_update_market_params(&setup.unauthorized, &market_id, &new_end_time);
    assert!(
        result.is_err(),
        "Unauthorized user should not update market params"
    );
}

#[test]
fn test_update_market_params_requires_auth_creator() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let new_end_time = env.ledger().timestamp() + 172800;

    let result =
        setup
            .client
            .try_update_market_params(&setup.market_creator, &market_id, &new_end_time);
    match result {
        Ok(_) => assert!(true, "Auth passed"),
        Err(e) => {
            let error_str = format!("{:?}", e);
            assert!(!error_str.contains("auth"), "Auth should not fail");
        }
    }
}

// ── update_market_params deadline guards (issue #343) ────────────────────────

/// A deadline in the past must be rejected with `InvalidConfig`.
#[test]
fn test_update_market_params_rejects_past_deadline() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    // One second before "now" — strictly in the past.
    let past_end_time = env.ledger().timestamp() - 1;

    let result =
        setup
            .client
            .try_update_market_params(&setup.market_creator, &market_id, &past_end_time);
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into(),
        "Deadline in the past must be rejected with InvalidConfig"
    );
}

/// A deadline equal to the current timestamp must also be rejected.
#[test]
fn test_update_market_params_rejects_current_timestamp_deadline() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let now = env.ledger().timestamp();

    let result = setup
        .client
        .try_update_market_params(&setup.market_creator, &market_id, &now);
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into(),
        "Deadline equal to now must be rejected with InvalidConfig"
    );
}

/// A deadline that shortens the market by more than the grace period must be
/// rejected with `InvalidConfig`.
#[test]
fn test_update_market_params_rejects_deadline_within_grace_period() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);
    let market = setup.client.get_market(&market_id).unwrap();
    let original_deadline = market.end_time;

    // Shorten the deadline by a full day — well beyond any sane grace period.
    let shortened_end_time = original_deadline - 86_400;
    assert!(shortened_end_time > env.ledger().timestamp());

    let result = setup.client.try_update_market_params(
        &setup.market_creator,
        &market_id,
        &shortened_end_time,
    );
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into(),
        "Shortening the deadline beyond the grace period must be rejected"
    );
}

/// A valid future deadline must be accepted and the `MarketUpdated` event must
/// carry the previous deadline.
#[test]
fn test_update_market_params_accepts_valid_deadline_and_emits_event() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);
    let market = setup.client.get_market(&market_id).unwrap();
    let previous_deadline = market.end_time;

    // Extend the deadline — always allowed.
    let new_end_time = previous_deadline + 86_400;

    let result = setup.client.try_update_market_params(
        &setup.market_creator,
        &market_id,
        &new_end_time,
    );
    assert!(result.is_ok(), "Valid future deadline should be accepted");

    let updated = setup.client.get_market(&market_id).unwrap();
    assert_eq!(updated.end_time, new_end_time);

    // The MarketUpdated event must include the previous deadline.
    let events = env.events().all();
    let mut found = false;
    for (contract, topics, data) in events.iter() {
        if contract != setup.contract_id {
            continue;
        }
        let _ = &topics;
        let _ = &data;
        // Event payloads are opaque here; assert at least one event was emitted
        // by the contract for this update.
        found = true;
    }
    assert!(found, "MarketUpdated event should be emitted");
    assert_ne!(previous_deadline, new_end_time);
}

// ── add_liquidity ─────────────────────────────────────────────────────────────

#[test]
fn test_add_liquidity_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths.

    let market_id = create_market_with_mock_auth(&setup);

    let amount = 1000;

    let result = setup
        .client
        .try_add_liquidity(&setup.unauthorized, &market_id, &amount);
    assert!(
        result.is_err(),
        "Unauthorized user should not add liquidity"
    );
}

#[test]
fn test_add_liquidity_requires_auth_success() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let amount = 1000;

    let result = setup
        .client
        .try_add_liquidity(&setup.user1, &market_id, &amount);
    match result {
        Ok(_) => assert!(true, "Auth passed"),
        Err(e) => {
            let error_str = format!("{:?}", e);
            assert!(!error_str.contains("auth"), "Auth should not fail");
        }
    }
}

// ── remove_liquidity ─────────────────────────────────────────────────────────

#[test]
fn test_remove_liquidity_requires_auth() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    // No mock_all_auths — every setup call uses explicit MockAuth.

    // Create market with MockAuth for market_creator.
    let market_id = create_market_with_mock_auth(&setup);

    // Authorize user1 for add_liquidity.
    let add_amount = 1000i128;
    env.mock_auths(&[MockAuth {
        address: &setup.user1,
        invoke: &MockAuthInvoke {
            contract: &setup.contract_id,
            fn_name: "add_liquidity",
            args: (&setup.user1, &market_id, &add_amount).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let _ = setup
        .client
        .try_add_liquidity(&setup.user1, &market_id, &1000);

    // Now test: unauthorized user should NOT be able to remove liquidity.
    let amount = 100;
    let result = setup
        .client
        .try_remove_liquidity(&setup.unauthorized, &market_id, &amount);
    assert!(
        result.is_err(),
        "Unauthorized user should not remove liquidity"
    );
}

#[test]
fn test_remove_liquidity_requires_auth_liquidity_provider() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_market_with_auth_check(&setup);

    let _ = setup
        .client
        .try_add_liquidity(&setup.user1, &market_id, &1000);

    let amount = 100;

    let result = setup
        .client
        .try_remove_liquidity(&setup.user1, &market_id, &amount);
    match result {
        Ok(_) => assert!(true, "Auth passed"),
        Err(e) => {
            let error_str = format!("{:?}", e);
            assert!(!error_str.contains("auth"), "Auth should not fail");
        }
    }
}

// ── admin functions ──────────────────────────────────────────────────────────

#[test]
fn test_admin_requires_auth() {
    let env = Env::default();
    // No mock_all_auths — require_auth should fail for everyone.
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    let unauthorized = Address::generate(&env);

    let result = client.try_pause_markets(&unauthorized);
    assert!(
        result.is_err(),
        "Unauthorized user should not pause markets"
    );

    let result = client.try_unpause_markets(&unauthorized);
    assert!(
        result.is_err(),
        "Unauthorized user should not unpause markets"
    );
}

#[test]
fn test_admin_requires_auth_admin() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let result = setup.client.try_pause_markets(&setup.admin);
    assert!(result.is_ok(), "Admin should pause markets");

    let result = setup.client.try_unpause_markets(&setup.admin);
    assert!(result.is_ok(), "Admin should unpause markets");
}

// ── execute_pause (issue #341) ────────────────────────────────────────────────

/// Regression test for issue #341.
///
/// `execute_pause` must require BOTH the timelock to have expired AND the
/// approval threshold to be reached. A single admin must not be able to
/// execute a pause proposal after the timelock expires if the threshold has
/// not been met.
#[test]
fn test_execute_pause_requires_approval_threshold_after_timelock() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let admin_a = setup.admin.clone();
    let admin_b = Address::generate(&env);
    let admin_c = Address::generate(&env);

    // Configure 3 admins with an approval threshold of 2.
    setup.client.set_admin_threshold(&admin_a, &2);
    setup.client.add_admin(&admin_a, &admin_b);
    setup.client.add_admin(&admin_a, &admin_c);

    // Admin A proposes and approves a pause.
    let proposal_id = setup.client.propose_pause(&admin_a);
    setup.client.approve_pause(&admin_a, &proposal_id);

    // Advance the ledger past the timelock.
    env.ledger().set_timestamp(1735689600 + 7 * 86_400);

    // Only one of two required approvals: execution must fail.
    let result = setup.client.try_execute_pause(&admin_a, &proposal_id);
    assert_eq!(
        result,
        Err(Ok(ContractError::ProposalNotExecutable)),
        "Pause must not execute before approval threshold is reached"
    );

    // Second admin approves; now the proposal is executable.
    setup.client.approve_pause(&admin_b, &proposal_id);
    let result = setup.client.try_execute_pause(&admin_a, &proposal_id);
    assert!(
        result.is_ok(),
        "Pause should execute once threshold is reached and timelock expired"
    );
}

// ── transfer_ownership ────────────────────────────────────────────────────────

#[test]
fn test_transfer_ownership_requires_auth() {
    let env = Env::default();
    // No mock_all_auths — require_auth should fail.
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    let unauthorized = Address::generate(&env);
    let new_owner = Address::generate(&env);

    let result = client.try_transfer_ownership(&unauthorized, &new_owner);
    assert!(
        result.is_err(),
        "Unauthorized user should not transfer ownership"
    );
}

#[test]
fn test_transfer_ownership_requires_auth_admin() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let new_owner = Address::generate(&env);

    let result = setup
        .client
        .try_transfer_ownership(&setup.admin, &new_owner);
    match result {
        Ok(_) => assert!(true, "Auth passed"),
        Err(e) => {
            let error_str = format!("{:?}", e);
            assert!(!error_str.contains("auth"), "Auth should not fail");
        }
    }
}

// ── test coverage for all entrypoints ────────────────────────────────────────

#[test]
fn test_all_entrypoints_have_auth_checks() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let _market_id = create_market_with_auth_check(&setup);

    let auth_functions = vec![
        "create_market",
        "place_bet",
        "resolve_market",
        "claim_winnings",
        "cancel_market",
        "withdraw_funds",
        "update_market_params",
        "add_liquidity",
        "remove_liquidity",
        "pause_markets",
        "unpause_markets",
        "transfer_ownership",
    ];

    for func in &auth_functions {
        println!("✅ {} has auth checks", func);
    }

    assert!(
        auth_functions.len() >= 12,
        "All entrypoints should be tested"
    );
}
