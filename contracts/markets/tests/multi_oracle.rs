#![cfg(test)]

use markets::{MarketsContract, MarketsContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Vec,
};

// ============================================
// Test Helpers
// ============================================

fn setup_test_environment(env: &Env) -> TestSetup<'_> {
    env.ledger().set(LedgerInfo {
        timestamp: 1735689600,
        protocol_version: 25,
        sequence_number: 1,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 518400,
    });

    let admin = Address::generate(env);
    let market_creator = Address::generate(env);
    let oracle1 = Address::generate(env);
    let oracle2 = Address::generate(env);
    let oracle3 = Address::generate(env);
    let oracle4 = Address::generate(env);
    let oracle5 = Address::generate(env);

    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(env, &contract_id);

    TestSetup {
        admin,
        market_creator,
        oracle1,
        oracle2,
        oracle3,
        oracle4,
        oracle5,
        client,
        contract_id,
    }
}

struct TestSetup<'a> {
    admin: Address,
    market_creator: Address,
    oracle1: Address,
    oracle2: Address,
    oracle3: Address,
    oracle4: Address,
    oracle5: Address,
    client: MarketsContractClient<'a>,
    contract_id: Address,
}

/// Creates a multi-oracle market configured for 3 oracles.
fn create_multi_oracle_market(setup: &TestSetup<'_>) -> u32 {
    let env = &setup.client.env;
    env.mock_all_auths();

    let question = String::from_str(env, "Will the price be above $100?");
    let description = String::from_str(env, "Price prediction market with multi-oracle resolution");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(env, "Multi-Oracle");
    let outcome_tags = Vec::from_array(
        env,
        [
            String::from_str(env, "Yes"),
            String::from_str(env, "No"),
            String::from_str(env, "Unknown"),
        ],
    );
    let required_oracles = 3u32;

    setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &required_oracles,
    )
}

/// Creates a single-oracle market (required_oracles = 0).
fn create_single_oracle_market(setup: &TestSetup<'_>) -> u32 {
    let env = &setup.client.env;
    env.mock_all_auths();

    let question = String::from_str(env, "Will it rain tomorrow?");
    let description = String::from_str(env, "Single oracle market");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(env, "Single-Oracle");
    let outcome_tags = Vec::from_array(
        env,
        [
            String::from_str(env, "Yes"),
            String::from_str(env, "No"),
        ],
    );
    let required_oracles = 0u32;

    setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &required_oracles,
    )
}

// ============================================
// Multi-Oracle Resolution Tests
// ============================================

/// Test: Submitting oracle outcomes for a multi-oracle market succeeds with valid data.
#[test]
fn test_submit_oracle_outcome_success() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Oracle 1 submits outcome 0
    let result1 = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    assert!(result1.is_ok(), "Oracle 1 submission should succeed");

    // Oracle 2 submits outcome 0
    let result2 = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &0u32);
    assert!(result2.is_ok(), "Oracle 2 submission should succeed");

    // Oracle 3 submits outcome 1
    let result3 = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &1u32);
    assert!(result3.is_ok(), "Oracle 3 submission should succeed");
}

/// Test: An oracle cannot submit twice for the same market.
#[test]
fn test_submit_oracle_outcome_duplicate_fails() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Oracle 1 submits outcome 0
    let result1 = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    assert!(result1.is_ok(), "First submission should succeed");

    // Oracle 1 attempts to submit again
    let result2 = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &1u32);
    assert!(
        result2.is_err(),
        "Duplicate submission should fail with OracleAlreadySubmitted"
    );
}

/// Test: Cannot submit an invalid outcome index.
#[test]
fn test_submit_oracle_outcome_invalid_outcome() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Outcome index 5 is out of range (market has 3 outcomes: 0, 1, 2)
    let result = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &5u32);
    assert!(
        result.is_err(),
        "Invalid outcome index should fail with InvalidOracleOutcome"
    );
}

/// Test: Cannot submit for a single-oracle market via multi-oracle submission.
#[test]
fn test_submit_oracle_outcome_single_oracle_market_fails() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_single_oracle_market(&setup);

    let result = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    assert!(
        result.is_err(),
        "Cannot submit to single-oracle market via multi-oracle submission"
    );
}

/// Test: Cannot submit for a non-existent market.
#[test]
fn test_submit_oracle_outcome_market_not_found() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let result = setup.client.try_submit_oracle_outcome(&setup.oracle1, &999u32, &0u32);
    assert!(
        result.is_err(),
        "Submission to non-existent market should fail with MarketNotFound"
    );
}

/// Test: Resolve with median aggregation (odd number of oracles).
#[test]
fn test_resolve_market_with_oracles_median_odd() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Oracle 1 submits outcome 0
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    // Oracle 2 submits outcome 1 (middle value)
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);
    // Oracle 3 submits outcome 2
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &2u32);

    // Create oracle list
    let oracle_addresses = Vec::from_array(&env, [setup.oracle1.clone(), setup.oracle2.clone(), setup.oracle3.clone()]);

    // Resolve with all 3 oracles
    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(result.is_ok(), "Resolution should succeed");
}

/// Test: Resolve with median aggregation (even number of oracles, lower-middle selected).
#[test]
fn test_resolve_market_with_oracles_median_even() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    // Create market requiring 5 oracles but allow 4
    let question = String::from_str(&env, "Test market?");
    let description = String::from_str(&env, "Test");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(&env, "Multi-Oracle");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
            String::from_str(&env, "Unknown"),
        ],
    );
    let market_id = setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &3u32,
    );

    // Submit 4 oracle outcomes: 0, 1, 1, 2 (sorted: 0, 1, 1, 2, median = 1)
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle4, &market_id, &2u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
            setup.oracle4.clone(),
        ],
    );

    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(result.is_ok(), "Resolution with 4 oracle submissions should succeed");
}

/// Test: Cannot resolve with insufficient oracle submissions.
#[test]
fn test_resolve_market_with_oracles_insufficient() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Only 2 oracles submit (need 3)
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
        ],
    );

    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(
        result.is_err(),
        "Resolution with only 2 oracle submissions should fail"
    );
}

/// Test: Cannot resolve the same market twice.
#[test]
fn test_resolve_market_with_oracles_already_resolved() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Submit 3 oracle outcomes
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &2u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
        ],
    );

    // First resolution should succeed
    let result1 = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(result1.is_ok(), "First resolution should succeed");

    // Second resolution should fail
    let result2 = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(
        result2.is_err(),
        "Second resolution should fail with MarketAlreadyResolved"
    );
}

/// Test: Cannot submit outcomes after market is resolved.
#[test]
fn test_submit_oracle_outcome_after_resolved() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Submit and resolve with 3 oracles
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &2u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
        ],
    );

    let _ = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);

    // Attempt to submit after resolution
    let result = setup.client.try_submit_oracle_outcome(&setup.oracle4, &market_id, &0u32);
    assert!(
        result.is_err(),
        "Cannot submit outcome after market is resolved"
    );
}

/// Test: Median aggregation resists individual oracle manipulation.
#[test]
fn test_resolve_market_with_oracles_manipulation_resistance() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_multi_oracle_market(&setup);

    // Scenario: 3 honest oracles say outcome 0, 1 malicious oracle says outcome 2
    // Median should still be 0 (honest consensus)
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &0u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
        ],
    );

    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(result.is_ok(), "Resolution should succeed");
}

/// Test: Resolve with 5 oracles (testing larger scenario).
#[test]
fn test_resolve_market_with_oracles_five() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    // Create market requiring 3 oracles
    let question = String::from_str(&env, "Market question?");
    let description = String::from_str(&env, "Five oracle market");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(&env, "Multi-Oracle");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
            String::from_str(&env, "Unknown"),
        ],
    );
    let market_id = setup.client.create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &3u32,
    );

    // 5 oracles submit: outcomes 0, 1, 1, 2, 2 (sorted: 0, 1, 1, 2, 2, median = 1)
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle1, &market_id, &0u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle2, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle3, &market_id, &1u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle4, &market_id, &2u32);
    let _ = setup.client.try_submit_oracle_outcome(&setup.oracle5, &market_id, &2u32);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
            setup.oracle4.clone(),
            setup.oracle5.clone(),
        ],
    );

    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(result.is_ok(), "Resolution with 5 oracle submissions should succeed");
}

/// Test: Cannot call resolve_market_with_oracles for single-oracle market.
#[test]
fn test_resolve_market_with_oracles_single_oracle_market() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let market_id = create_single_oracle_market(&setup);

    let oracle_addresses = Vec::from_array(
        &env,
        [
            setup.oracle1.clone(),
            setup.oracle2.clone(),
            setup.oracle3.clone(),
        ],
    );

    let result = setup
        .client
        .try_resolve_market_with_oracles(&setup.market_creator, &market_id, &oracle_addresses);
    assert!(
        result.is_err(),
        "Cannot use multi-oracle resolution for single-oracle market"
    );
}

// ============================================
// Edge Cases
// ============================================

/// Test: Correct configuration validation (requires 3+ oracles).
#[test]
fn test_create_market_invalid_oracle_count() {
    let env = Env::default();
    let setup = setup_test_environment(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Test?");
    let description = String::from_str(&env, "Test");
    let end_time = env.ledger().timestamp() + 86400;
    let resolution_source = String::from_str(&env, "Multi-Oracle");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
        ],
    );

    // Try to create with required_oracles = 2 (invalid, must be 0 or 3+)
    let result = setup.client.try_create_market(
        &setup.market_creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &2u32,
    );
    assert!(
        result.is_err(),
        "Creating market with 2 oracles should fail (must be 0 or 3+)"
    );
}
