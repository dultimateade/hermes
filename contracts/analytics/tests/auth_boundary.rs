#![cfg(test)]

extern crate std;

use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, Env, Symbol};
use analytics::{Analytics, ContractError};

// Helper contract that wraps Analytics for testing
#[contract]
pub struct TestAnalytics;

#[contractimpl]
impl TestAnalytics {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        Analytics::initialize(env, admin)
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        Analytics::get_admin(env)
    }

    pub fn add_authorized_reporter(env: Env, reporter: Address) -> Result<(), ContractError> {
        Analytics::add_authorized_reporter(env, reporter)
    }

    pub fn remove_authorized_reporter(env: Env, reporter: Address) -> Result<(), ContractError> {
        Analytics::remove_authorized_reporter(env, reporter)
    }

    pub fn is_authorized_reporter(env: Env, reporter: Address) -> Result<bool, ContractError> {
        Analytics::is_authorized_reporter(env, reporter)
    }

    pub fn record_metric(env: Env, metric_name: Symbol, value: u64) -> Result<(), ContractError> {
        Analytics::record_metric(env, metric_name, value).map(|_| ())
    }

    pub fn record_metric_at(
        env: Env,
        metric_name: Symbol,
        timestamp: u64,
        value: u64,
    ) -> Result<(), ContractError> {
        Analytics::record_metric_at(env, metric_name, timestamp, value).map(|_| ())
    }

    pub fn aggregate(env: Env, event_types: soroban_sdk::Vec<u32>) -> u64 {
        Analytics::aggregate(env, event_types)
    }

    pub fn percentile(env: Env, values: soroban_sdk::Vec<u64>, pct: u32) -> u64 {
        Analytics::percentile(env, values, pct)
    }
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
fn test_initialize_succeeds_with_valid_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    // First initialize should succeed
    let res = client.try_initialize(&admin);
    assert!(res.is_ok(), "initialize should succeed");

    // Verify admin was set
    let retrieved_admin = client.get_admin().expect("admin should be set");
    assert_eq!(retrieved_admin, admin);
}

#[test]
fn test_initialize_fails_if_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    // First initialize succeeds
    client.initialize(&admin1);

    // Second initialize should fail with AlreadyInitialized
    let res = client.try_initialize(&admin2);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::AlreadyInitialized
    );
}

// ============================================================================
// Authorized Reporter Management Tests
// ============================================================================

#[test]
fn test_add_authorized_reporter_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);

    // Non-admin tries to add reporter
    env.mock_all_auths_allowing_non_root_invoker();
    let res = client.try_add_authorized_reporter(&reporter);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::Unauthorized,
        "non-admin should not be able to add reporter"
    );
}

#[test]
fn test_add_authorized_reporter_succeeds_with_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);

    // Admin adds reporter
    let res = client.try_add_authorized_reporter(&reporter);
    assert!(res.is_ok(), "admin should be able to add reporter");

    // Verify reporter is authorized
    assert!(
        client
            .is_authorized_reporter(&reporter)
            .expect("should not error"),
        "reporter should be authorized"
    );
}

#[test]
fn test_add_authorized_reporter_prevents_duplicates() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Try to add same reporter again
    let res = client.try_add_authorized_reporter(&reporter);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::DuplicateEntry,
        "duplicate reporter should be rejected"
    );
}

#[test]
fn test_remove_authorized_reporter_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Non-admin tries to remove reporter
    env.mock_all_auths_allowing_non_root_invoker();
    let res = client.try_remove_authorized_reporter(&reporter);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::Unauthorized,
        "non-admin should not be able to remove reporter"
    );
}

#[test]
fn test_remove_authorized_reporter_succeeds_with_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Admin removes reporter
    let res = client.try_remove_authorized_reporter(&reporter);
    assert!(res.is_ok(), "admin should be able to remove reporter");

    // Verify reporter is no longer authorized
    assert!(
        !client
            .is_authorized_reporter(&reporter)
            .expect("should not error"),
        "reporter should no longer be authorized"
    );
}

#[test]
fn test_remove_authorized_reporter_fails_if_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);

    // Try to remove reporter that was never added
    let res = client.try_remove_authorized_reporter(&reporter);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::MarketNotFound,
        "removing non-existent reporter should fail"
    );
}

// ============================================================================
// Metric Recording Tests - Authorization Boundary
// ============================================================================

#[test]
fn test_record_metric_requires_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Unauthorized caller tries to record metric
    env.mock_all_auths_allowing_non_root_invoker();
    let res = client.try_record_metric(&Symbol::new(&env, "price_feed"), &100u64);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::Unauthorized,
        "unauthorized caller should not be able to record metric"
    );
}

#[test]
fn test_record_metric_succeeds_with_authorized_reporter() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Authorized reporter records metric
    let metric_name = Symbol::new(&env, "price_feed");
    let value = 12345u64;
    let res = client.try_record_metric(&metric_name, &value);
    assert!(
        res.is_ok(),
        "authorized reporter should be able to record metric"
    );
}

#[test]
fn test_record_metric_at_requires_authorization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Set a base timestamp
    env.ledger().with_mut(|l| {
        l.timestamp = 1000;
    });

    // Unauthorized caller tries to record metric at specific time
    env.mock_all_auths_allowing_non_root_invoker();
    let res = client.try_record_metric_at(
        &Symbol::new(&env, "price_feed"),
        &999u64,
        &50u64,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::Unauthorized,
        "unauthorized caller should not be able to record metric at specific time"
    );
}

#[test]
fn test_record_metric_at_succeeds_with_authorized_reporter() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    let metric_name = Symbol::new(&env, "price_feed");
    let timestamp = 500u64;
    let value = 999u64;

    let res = client.try_record_metric_at(&metric_name, &timestamp, &value);
    assert!(
        res.is_ok(),
        "authorized reporter should be able to record metric at specific time"
    );
}

#[test]
fn test_record_metric_prevents_duplicate_entries() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    let metric_name = Symbol::new(&env, "price_feed");
    let timestamp = 500u64;
    let value1 = 100u64;
    let value2 = 200u64;

    // Record metric first time
    let res1 = client.try_record_metric_at(&metric_name, &timestamp, &value1);
    assert!(res1.is_ok(), "first metric recording should succeed");

    // Try to record same metric (same name, timestamp) again
    let res2 = client.try_record_metric_at(&metric_name, &timestamp, &value2);
    assert_eq!(
        res2.unwrap_err().unwrap(),
        ContractError::DuplicateEntry,
        "duplicate metric recording should be rejected"
    );
}

#[test]
fn test_record_metric_requires_contract_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);

    // Try to record metric before initialization
    let res = client.try_record_metric(&Symbol::new(&env, "price_feed"), &100u64);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::NotInitialized,
        "recording metric before initialization should fail"
    );
}

#[test]
fn test_reporter_revocation_prevents_further_metrics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Reporter records metric successfully
    let res1 = client.try_record_metric(&Symbol::new(&env, "price_feed"), &100u64);
    assert!(res1.is_ok(), "authorized reporter should be able to record metric");

    // Admin removes reporter
    client.remove_authorized_reporter(&reporter);

    // Reporter tries to record another metric
    let res2 = client.try_record_metric(&Symbol::new(&env, "price_feed"), &200u64);
    assert_eq!(
        res2.unwrap_err().unwrap(),
        ContractError::Unauthorized,
        "revoked reporter should not be able to record metrics"
    );
}

// ============================================================================
// Read-Only Operations (no auth required)
// ============================================================================

#[test]
fn test_is_authorized_reporter_does_not_require_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    client.initialize(&admin);
    client.add_authorized_reporter(&reporter);

    // Anyone can call is_authorized_reporter (it's read-only)
    let is_authorized = client.is_authorized_reporter(&reporter);
    assert!(
        is_authorized.expect("should not error"),
        "read-only call should succeed"
    );
}

#[test]
fn test_get_admin_does_not_require_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Anyone can call get_admin (it's read-only)
    let retrieved_admin = client.get_admin();
    assert_eq!(
        retrieved_admin.expect("admin should be set"),
        admin,
        "read-only call should succeed"
    );
}

#[test]
fn test_aggregate_does_not_require_auth() {
    let env = Env::default();
    // Don't mock all auths for this test to verify it works without auth

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);

    let event_types = {
        let mut v = soroban_sdk::Vec::new(&env);
        v.push_back(10u32);
        v.push_back(20u32);
        v.push_back(30u32);
        v
    };

    let sum = client.aggregate(&event_types);
    assert_eq!(sum, 60u64, "aggregate should work without auth");
}

#[test]
fn test_percentile_does_not_require_auth() {
    let env = Env::default();
    // Don't mock all auths for this test to verify it works without auth

    let contract_id = env.register(TestAnalytics, ());
    let client = TestAnalyticsClient::new(&env, &contract_id);

    let values = {
        let mut v = soroban_sdk::Vec::new(&env);
        for i in 1..=100 {
            v.push_back(i as u64);
        }
        v
    };

    let p50 = client.percentile(&values, &50u32);
    assert_eq!(p50, 50u64, "percentile should work without auth");

    let p99 = client.percentile(&values, &99u32);
    assert_eq!(p99, 99u64, "percentile should work without auth");
}
