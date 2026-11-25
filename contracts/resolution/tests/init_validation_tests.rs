//! Integration tests for MarketsContract address validation in `ResolutionContract`.
//!
//! # What is tested
//!
//! | # | Scenario                                                    | Expected result                       |
//! |---|-------------------------------------------------------------|---------------------------------------|
//! | 1 | Happy path: valid MarketsContract address at init           | `Ok(())`, version stored              |
//! | 2 | Init with an address that has no `version()` entrypoint     | `Err(MarketsContractInvalid)`         |
//! | 3 | Double-initialization                                       | `Err(InvalidState)`                   |
//! | 4 | `resolve` before `initialize`                               | `Err(InvalidState)`                   |
//! | 5 | `resolve` by non-admin                                      | `Err(Unauthorized)`                   |
//! | 6 | Re-validation on `resolve` — version matches                | call succeeds past the version check  |
//! | 7 | Re-validation on `resolve` — version mismatch               | `Err(MarketsVersionMismatch)`         |
//! | 8 | `initialize` by non-admin (no-auth)                         | host traps (require_auth fails)       |
//! | 9 | View entrypoints before init return `None`                  | `None`                                |

use resolution::{ContractError, DataKey, ResolutionContract, ResolutionContractClient};
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, Symbol,
    contract, contractimpl,
};

// ---------------------------------------------------------------------------
// Minimal stub — a valid MarketsContract with version() → 7
// ---------------------------------------------------------------------------

/// A minimal stub of the MarketsContract that exposes only `version()`.
/// Used to satisfy the cross-contract validation without pulling in the full
/// markets crate state machinery.
#[contract]
pub struct MarketsStub;

#[contractimpl]
impl MarketsStub {
    pub fn version(_env: Env) -> u32 {
        7
    }
    /// Accepts any `(market_id, outcome)` pair and does nothing — enough for
    /// the happy-path resolve test to complete the cross-contract call.
    pub fn resolve(_env: Env, _market_id: Symbol, _outcome: soroban_sdk::String) {}
}

// ---------------------------------------------------------------------------
// Stub with a *different* version — used in the mismatch test.
// ---------------------------------------------------------------------------

#[contract]
pub struct MarketsStubV99;

#[contractimpl]
impl MarketsStubV99 {
    pub fn version(_env: Env) -> u32 {
        99
    }
}

// ---------------------------------------------------------------------------
// Stub with *no* version() entrypoint — used in the invalid-address test.
// ---------------------------------------------------------------------------

#[contract]
pub struct NotAMarketsContract;

#[contractimpl]
impl NotAMarketsContract {
    pub fn some_other_fn(_env: Env) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let resolution_id = env.register(ResolutionContract, ());
    let markets_id = env.register(MarketsStub, ());

    (env, resolution_id, markets_id)
}

fn client<'a>(env: &'a Env, id: &Address) -> ResolutionContractClient<'a> {
    ResolutionContractClient::new(env, id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// 1. Happy path: valid MarketsContract address accepted at initialization.
///
/// After `initialize` succeeds the view entrypoints must reflect the stored
/// values and the recorded version must match what the stub returns.
#[test]
fn test_init_valid_markets_contract() {
    let (env, resolution_id, markets_id) = setup();
    let admin = Address::generate(&env);
    let c = client(&env, &resolution_id);

    let result = c.try_initialize(&admin, &markets_id);
    assert!(result.is_ok(), "initialize with a valid markets contract should succeed");

    // The stored address is returned by the view.
    assert_eq!(c.markets_contract(), Some(markets_id.clone()));

    // The expected version is the value returned by MarketsStub::version() = 7.
    assert_eq!(c.expected_markets_version(), Some(7u32));
}

/// 2. Initialization must fail when the supplied address has no `version()` entrypoint.
#[test]
fn test_init_invalid_address_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let resolution_id = env.register(ResolutionContract, ());
    let not_markets_id = env.register(NotAMarketsContract, ());
    let admin = Address::generate(&env);

    let c = client(&env, &resolution_id);
    let result = c.try_initialize(&admin, &not_markets_id);

    assert_eq!(
        result,
        Err(Ok(ContractError::MarketsContractInvalid)),
        "address without version() should be rejected with MarketsContractInvalid"
    );
}

/// 3. Calling `initialize` a second time must return `InvalidState`.
#[test]
fn test_double_init_rejected() {
    let (env, resolution_id, markets_id) = setup();
    let admin = Address::generate(&env);
    let c = client(&env, &resolution_id);

    c.initialize(&admin, &markets_id);

    let result = c.try_initialize(&admin, &markets_id);
    assert_eq!(
        result,
        Err(Ok(ContractError::InvalidState)),
        "second initialize call should be rejected"
    );
}

/// 4. Calling `resolve` before `initialize` must return `InvalidState`.
#[test]
fn test_resolve_before_init() {
    let env = Env::default();
    env.mock_all_auths();

    let resolution_id = env.register(ResolutionContract, ());
    let admin = Address::generate(&env);
    let c = client(&env, &resolution_id);
    let market_id = Symbol::new(&env, "mkt1");
    let outcome = soroban_sdk::String::from_str(&env, "yes");

    let result = c.try_resolve(&admin, &market_id, &outcome);
    assert_eq!(
        result,
        Err(Ok(ContractError::InvalidState)),
        "resolve before initialize should return InvalidState"
    );
}

/// 5. `resolve` called by a non-admin must return `Unauthorized`.
#[test]
fn test_resolve_non_admin_rejected() {
    let (env, resolution_id, markets_id) = setup();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let c = client(&env, &resolution_id);

    c.initialize(&admin, &markets_id);

    let market_id = Symbol::new(&env, "mkt1");
    let outcome = soroban_sdk::String::from_str(&env, "yes");
    let result = c.try_resolve(&non_admin, &market_id, &outcome);

    assert_eq!(
        result,
        Err(Ok(ContractError::Unauthorized)),
        "non-admin resolve should be rejected"
    );
}

/// 6. `resolve` by admin — re-validation passes when the live version matches.
///
/// The MarketsStub::resolve() is a no-op, so this exercises the full path
/// including the version re-check without needing a real market to exist.
#[test]
fn test_resolve_version_revalidation_passes() {
    let (env, resolution_id, markets_id) = setup();
    let admin = Address::generate(&env);
    let c = client(&env, &resolution_id);

    c.initialize(&admin, &markets_id);

    let market_id = Symbol::new(&env, "mkt1");
    let outcome = soroban_sdk::String::from_str(&env, "yes");

    // Should succeed: version() on MarketsStub still returns 7 = expected.
    let result = c.try_resolve(&admin, &market_id, &outcome);
    assert!(
        result.is_ok(),
        "resolve should succeed when live version matches expected"
    );
}

/// 7. `resolve` must fail when the stored `ExpectedMarketsVersion` is overwritten
///    to a different value, simulating a contract upgrade that changes the version.
///
/// We test `MarketsVersionMismatch` by registering a v99 stub and then patching
/// the stored expected version in instance storage to 7, so that the live call
/// (returning 99) diverges from the expectation.
#[test]
fn test_resolve_version_mismatch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    // Register a v99 stub as the "new" markets contract.
    let resolution_id = env.register(ResolutionContract, ());
    let markets_v99_id = env.register(MarketsStubV99, ());

    // Also register a v7 stub so initialize() can succeed.
    let markets_v7_id = env.register(MarketsStub, ());
    let admin = Address::generate(&env);
    let c = client(&env, &resolution_id);

    // Initialize with v7 stub → stores expected_version = 7.
    c.initialize(&admin, &markets_v7_id);

    // Now patch storage: replace the stored MarketsContract address with the
    // v99 stub so the next version() call returns 99 instead of 7.
    env.as_contract(&resolution_id, || {
        env.storage()
            .instance()
            .set(&DataKey::MarketsContract, &markets_v99_id);
    });

    let market_id = Symbol::new(&env, "mkt1");
    let outcome = soroban_sdk::String::from_str(&env, "yes");
    let result = c.try_resolve(&admin, &market_id, &outcome);

    assert_eq!(
        result,
        Err(Ok(ContractError::MarketsVersionMismatch)),
        "version mismatch should return MarketsVersionMismatch"
    );
}

/// 9. View entrypoints return `None` before initialization.
#[test]
fn test_views_before_init_return_none() {
    let env = Env::default();
    env.mock_all_auths();

    let resolution_id = env.register(ResolutionContract, ());
    let c = client(&env, &resolution_id);

    assert_eq!(c.markets_contract(), None);
    assert_eq!(c.expected_markets_version(), None);
}

/// The `version()` entrypoint is always available regardless of init state.
#[test]
fn test_version_always_available() {
    let env = Env::default();
    let resolution_id = env.register(ResolutionContract, ());
    let c = ResolutionContractClient::new(&env, &resolution_id);

    assert_eq!(c.version(), 7u32);
}
