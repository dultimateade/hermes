#![cfg(test)]

use fees::{ContractError, FeeConfig, FeesContract, FeesContractClient};
use soroban_sdk::{contract, contractimpl, testutils::Address as _, xdr::ToXdr, Address, Env};
use std::ops::Deref;

#[contract]
struct MockAuthorizedCaller;

#[contractimpl]
impl MockAuthorizedCaller {
    pub fn record_fee(
        env: Env,
        fees_contract: Address,
        caller: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        match FeesContractClient::new(&env, &fees_contract).try_record_fee(&caller, &amount) {
            Ok(()) => Ok(()),
            Err(Ok(error)) => Err(error),
            Err(Err(_)) => panic!("Fee contract invocation failed unexpectedly"),
        }
    }
}

struct InitializedFees<'a> {
    client: FeesContractClient<'a>,
    contract_id: Address,
}

impl<'a> Deref for InitializedFees<'a> {
    type Target = FeesContractClient<'a>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

// ============================================================
// Test Helpers
// ============================================================

/// Registers the contract and initializes it with the given admin.
/// The auth snapshot from initialization is drained so it does not
/// interfere with subsequent assertions on `env.auths()`.
///
/// Note: `mock_all_auths()` is sticky on this `Env`. All
/// `require_auth()` calls will pass for the rest of the test.
/// Tests that need to verify `require_auth()` rejection (as opposed
/// to admin authorization) must use a fresh `Env`.
fn register_and_initialize<'a>(env: &Env, admin: &Address) -> InitializedFees<'a> {
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(env, &contract_id);
    env.mock_all_auths();
    client.initialize(admin);
    // Drain auth snapshot from initialization
    let _ = env.auths();
    InitializedFees {
        client,
        contract_id,
    }
}

fn invoke_as_authorized_caller<T>(
    env: &Env,
    fees: &InitializedFees,
    amount: i128,
    invoke: impl FnOnce(&MockAuthorizedCallerClient, &Address, &Address, &i128) -> T,
) -> (T, Address) {
    let caller_id = env.register(MockAuthorizedCaller, ());
    fees.authorize_caller(&fees.get_admin(), &caller_id);
    let _ = env.auths();

    let caller = MockAuthorizedCallerClient::new(env, &caller_id);
    let result = invoke(&caller, &fees.contract_id, &caller_id, &amount);
    (result, caller_id)
}

fn default_fee_config() -> FeeConfig {
    FeeConfig {
        platform_fee_percentage: 500, // 5.00%
        creation_fee: 100,
        min_fee_amount: 10,
        max_fee_amount: 1_000_000,
        collection_threshold: 50_000,
        fees_enabled: true,
    }
}

/// Compute the sha256 hash of a FeeConfig's XDR encoding, matching what
/// the contract verifies in `update_fee_config`.
fn fee_config_hash(env: &Env, config: &FeeConfig) -> soroban_sdk::BytesN<32> {
    let bytes = config.to_xdr(env);
    env.crypto().sha256(&bytes)
}

// ============================================================
// Auth Boundary Tests — initialize
// ============================================================

/// `initialize` is the only entrypoint that can test low-level
/// `require_auth()` failure directly. All other state-changing
/// entrypoints need the contract already initialized, which
/// requires `mock_all_auths()` to be active. Their tests instead
/// verify admin authorization via the `assert_is_admin` guard.
#[test]
fn test_initialize_requires_auth() {
    let env = Env::default();
    // Do NOT mock_all_auths — require_auth should fail as a host error
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    let result = client.try_initialize(&admin);
    assert!(result.is_err(), "initialize should require auth");
}

#[test]
fn test_initialize_succeeds_with_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
}

#[test]
fn test_initialize_cannot_reinitialize() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let result = client.try_initialize(&admin);
    match result {
        Err(Ok(ContractError::InvalidState)) => {} // Expected
        other => panic!(
            "Re-initialization should fail with InvalidState, got: {:?}",
            other
        ),
    }
}

// ============================================================
// Auth Boundary Tests — update_fee_config
// ============================================================

#[test]
fn test_update_fee_config_rejects_hash_mismatch() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let config = default_fee_config();

    // Commit a hash for a *different* (wrong) config
    let wrong_config = FeeConfig {
        platform_fee_percentage: 100,
        creation_fee: 0,
        min_fee_amount: 1,
        max_fee_amount: 1_000_000,
        collection_threshold: 50_000,
        fees_enabled: true,
    };
    let wrong_hash = fee_config_hash(&env, &wrong_config);
    client.commit_fee_config(&admin, &wrong_hash);

    // Reveal with a different config — must be rejected
    let result = client.try_update_fee_config(&admin, &config);
    assert!(
        matches!(result, Err(Ok(ContractError::CommitHashMismatch))),
        "update_fee_config must reject configs that don't match the committed hash, got: {:?}",
        result
    );
}

#[test]
fn test_update_fee_config_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);

    let config = default_fee_config();
    let result = client.try_update_fee_config(&non_admin, &config);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_update_fee_config_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let config = default_fee_config();
    let hash = fee_config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    let _ = env.auths();
    client.update_fee_config(&admin, &config);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
}

#[test]
fn test_update_fee_config_rejects_invalid_percentage() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let mut config = default_fee_config();
    config.platform_fee_percentage = 20_000; // > 10_000

    let result = client.try_update_fee_config(&admin, &config);
    match result {
        Err(Ok(ContractError::FeePercentageTooHigh)) => {} // Expected
        other => panic!("Should reject excessive fee percentage, got: {:?}", other),
    }
}

#[test]
fn test_update_fee_config_rejects_negative_values() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let mut config = default_fee_config();
    config.creation_fee = -1;

    let result = client.try_update_fee_config(&admin, &config);
    match result {
        Err(Ok(ContractError::InvalidInput)) => {} // Expected
        other => panic!("Should reject negative creation fee, got: {:?}", other),
    }
}

#[test]
fn test_update_fee_config_rejects_zero_minimum_when_fees_enabled() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let mut config = default_fee_config();
    config.min_fee_amount = 0;
    config.max_fee_amount = 0;

    let result = client.try_update_fee_config(&admin, &config);
    assert!(matches!(result, Err(Ok(ContractError::InvalidInput))));

    config.fees_enabled = false;
    let hash = fee_config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);
}

#[test]
fn test_update_fee_config_requires_commit_and_apply_timelock() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let config = default_fee_config();

    let no_commit = client.try_update_fee_config(&admin, &config);
    assert!(matches!(
        no_commit,
        Err(Ok(ContractError::NoPendingFeeCommit))
    ));

    let hash = fee_config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);

    let early_apply = client.try_apply_fee_update(&admin);
    assert!(matches!(
        early_apply,
        Err(Ok(ContractError::FeeRevealTooEarly))
    ));

    env.ledger().with_mut(|ledger| {
        ledger.timestamp += 86_401;
    });
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config(), config);
}

// ============================================================
// Auth Boundary Tests — set_platform_fee
// ============================================================

#[test]
fn test_set_platform_fee_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);

    let result = client.try_set_platform_fee(&non_admin, &300);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_set_platform_fee_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let original_config = client.get_fee_config();
    client.set_platform_fee(&admin, &300);
    assert_eq!(client.get_fee_config(), original_config);

    let pending = client
        .get_pending_fee_config()
        .expect("platform fee update should be queued");
    assert_eq!(pending.config.platform_fee_percentage, 300);
    assert_eq!(pending.apply_eta, env.ledger().timestamp() + 86_400);

    let early_result = client.try_apply_fee_update(&admin);
    match early_result {
        Err(Ok(ContractError::FeeRevealTooEarly)) => {}
        other => panic!("Expected FeeRevealTooEarly, got: {:?}", other),
    }

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);

    env.ledger().with_mut(|ledger| {
        ledger.timestamp += 86_400;
    });
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config().platform_fee_percentage, 300);
}

#[test]
fn test_set_platform_fee_rejects_excessive() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let result = client.try_set_platform_fee(&admin, &11_000);
    match result {
        Err(Ok(ContractError::FeePercentageTooHigh)) => {} // Expected
        other => panic!("Should reject fee > 100%, got: {:?}", other),
    }
}

// ============================================================
// Auth Boundary Tests — collect_fees
// ============================================================

#[test]
fn test_collect_fees_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);

    let result = client.try_collect_fees(&non_admin);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_collect_fees_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // Record enough fees to meet the default collection threshold (100M stroops)
    let _ = invoke_as_authorized_caller(&env, &client, 150_000_000, |market, fees, caller, amount| {
        market.record_fee(fees, caller, amount)
    });
    // Drain auth from record_fee
    let _ = env.auths();

    client.collect_fees(&admin);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
}

#[test]
fn test_collect_fees_fails_below_threshold() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // Initial schedule should be Ready
    assert_eq!(
        client.get_withdrawal_schedule().status,
        fees::FeeWithdrawalStatus::Ready
    );

    // Record a tiny amount below the default threshold (100M stroops)
    let _ = invoke_as_authorized_caller(&env, &client, 100, |market, fees, caller, amount| {
        market.record_fee(fees, caller, amount)
    });

    let result = client.try_collect_fees(&admin);
    match result {
        Err(Ok(ContractError::BelowCollectionThreshold)) => {} // Expected
        other => panic!("Collection below threshold should fail, got: {:?}", other),
    }

    // A threshold rejection does not change the withdrawal schedule.
    assert_eq!(
        client.get_withdrawal_schedule().status,
        fees::FeeWithdrawalStatus::Ready
    );
}

#[test]
fn test_fee_withdrawal_status_unchanged_and_recovery() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // 1. Initial state is Ready
    let initial_schedule = client.get_withdrawal_schedule();
    assert_eq!(initial_schedule.status, fees::FeeWithdrawalStatus::Ready);

    // 2. Trigger failed collection attempt with zero balance
    let fail_result = client.try_collect_fees(&admin);
    assert_eq!(
        fail_result,
        Err(Ok(ContractError::BelowCollectionThreshold))
    );
    let unchanged_schedule = client.get_withdrawal_schedule();
    assert_eq!(unchanged_schedule.status, fees::FeeWithdrawalStatus::Ready);

    // 3. Deposit sufficient fees exceeding threshold (150M stroops > 100M threshold)
    let _ = invoke_as_authorized_caller(&env, &client, 150_000_000, |market, fees, caller, amount| {
        market.record_fee(fees, caller, amount)
    });

    // 4. Retry collection and verify successful transition to Completed
    let success_result = client.try_collect_fees(&admin);
    assert!(success_result.is_ok());

    let completed_schedule = client.get_withdrawal_schedule();
    assert_eq!(
        completed_schedule.status,
        fees::FeeWithdrawalStatus::Completed
    );
    assert_eq!(client.get_collected_fees(), 0);
}

// ============================================================
// Auth Boundary Tests — pause_fees
// ============================================================

#[test]
fn test_pause_fees_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);

    let result = client.try_pause_fees(&non_admin);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_pause_fees_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    client.pause_fees(&admin);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
    assert!(client.is_paused());
}

#[test]
fn test_pause_fees_prevents_record_fee() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    client.pause_fees(&admin);

    let (result, _) =
        invoke_as_authorized_caller(&env, &client, 1000, |market, fees, caller, amount| {
            market.try_record_fee(fees, caller, amount)
        });
    match result {
        Err(Ok(ContractError::FeesPaused)) => {} // Expected
        other => panic!(
            "Record fee should fail when fees are paused, got: {:?}",
            other
        ),
    }
}

// ============================================================
// Auth Boundary Tests — unpause_fees
// ============================================================

#[test]
fn test_unpause_fees_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);

    client.pause_fees(&admin);
    let _ = env.auths(); // drain pause auth

    let result = client.try_unpause_fees(&non_admin);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_unpause_fees_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    client.pause_fees(&admin);
    let _ = env.auths(); // drain pause auth

    client.unpause_fees(&admin);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
    assert!(!client.is_paused());
}

// ============================================================
// Auth Boundary Tests — transfer_admin
// ============================================================

#[test]
fn test_transfer_admin_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    let result = client.try_transfer_admin(&non_admin, &new_admin);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Non-admin should get Unauthorized, got: {:?}", other),
    }
}

#[test]
fn test_transfer_admin_succeeds_with_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let new_admin = Address::generate(&env);

    client.transfer_admin(&admin, &new_admin);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_transfer_admin_rejects_same_address() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let result = client.try_transfer_admin(&admin, &admin);
    match result {
        Err(Ok(ContractError::InvalidInput)) => {} // Expected
        other => panic!("Should reject transferring to same admin, got: {:?}", other),
    }
}

#[test]
fn test_transfer_admin_new_admin_can_operate() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let new_admin = Address::generate(&env);

    client.transfer_admin(&admin, &new_admin);

    // Old admin should no longer be able to operate
    let result = client.try_pause_fees(&admin);
    match result {
        Err(Ok(ContractError::Unauthorized)) => {} // Expected
        other => panic!("Old admin should get Unauthorized, got: {:?}", other),
    }

    // New admin should be able to operate
    client.pause_fees(&new_admin);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, new_admin);
}

// ============================================================
// Auth Boundary Tests — record_fee
// ============================================================

/// `record_fee` requires an admin-authorized contract to authenticate.
/// Because `mock_all_auths` must be active after initialization,
/// we verify the authorization via `env.auths()` — the contract
/// address must appear in the auth snapshot.
#[test]
fn test_record_fee_requires_authorized_caller_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let (_, authorized_caller) =
        invoke_as_authorized_caller(&env, &client, 1000, |market, fees, caller, amount| {
            market.record_fee(fees, caller, amount)
        });

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(
        auths[0].0, authorized_caller,
        "record_fee should authenticate the authorized contract"
    );
}

#[test]
fn test_record_fee_rejects_unauthorized_caller() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let unauthorized_caller = Address::generate(&env);

    let result = client.try_record_fee(&unauthorized_caller, &1000);
    assert!(
        matches!(result, Err(Ok(ContractError::Unauthorized))),
        "An unapproved caller must not record fees"
    );
    assert_eq!(client.get_collected_fees(), 0);
}

#[test]
fn test_admin_manages_authorized_callers() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let authorized = env.register(MockAuthorizedCaller, ());
    let non_admin = Address::generate(&env);

    assert!(!client.is_authorized_caller(&authorized));
    client.authorize_caller(&admin, &authorized);
    assert!(client.is_authorized_caller(&authorized));

    let result = client.try_revoke_caller(&non_admin, &authorized);
    assert!(matches!(result, Err(Ok(ContractError::Unauthorized))));
    assert!(client.is_authorized_caller(&authorized));

    client.revoke_caller(&admin, &authorized);
    assert!(!client.is_authorized_caller(&authorized));

    let caller = MockAuthorizedCallerClient::new(&env, &authorized);
    let result = caller.try_record_fee(&client.contract_id, &authorized, &1000);
    assert!(matches!(result, Err(Ok(ContractError::Unauthorized))));
}

#[test]
fn test_record_fee_rejects_zero_or_negative() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // Zero amount should fail
    let (result, _) = invoke_as_authorized_caller(&env, &client, 0, |market, fees, caller, amount| {
        market.try_record_fee(fees, caller, amount)
    });
    match result {
        Err(Ok(ContractError::InvalidInput)) => {} // Expected
        other => panic!("Zero fee should be rejected, got: {:?}", other),
    }

    // Negative amount should fail
    let (result, _) =
        invoke_as_authorized_caller(&env, &client, -100, |market, fees, caller, amount| {
            market.try_record_fee(fees, caller, amount)
        });
    match result {
        Err(Ok(ContractError::InvalidInput)) => {} // Expected
        other => panic!("Negative fee should be rejected, got: {:?}", other),
    }
}

// ============================================================
// Read-Only Entrypoints — No Auth Required
// ============================================================

#[test]
fn test_read_only_entrypoints_no_auth_required() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // All read-only functions should succeed without additional auth
    assert_eq!(client.version(), 7);

    let config = client.get_fee_config();
    assert_eq!(config.platform_fee_percentage, 200); // default

    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);

    assert_eq!(client.get_collected_fees(), 0);
    assert!(!client.is_paused());

    let schedule = client.get_withdrawal_schedule();
    assert_eq!(schedule.status, fees::FeeWithdrawalStatus::Ready);
}

#[test]
fn test_get_admin_fails_when_not_initialized() {
    let env = Env::default();
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(&env, &contract_id);

    let result = client.try_get_admin();
    match result {
        Err(Ok(ContractError::AdminNotSet)) => {} // Expected
        other => panic!(
            "get_admin should fail with AdminNotSet when uninitialized, got: {:?}",
            other
        ),
    }
}

// ============================================================
// Comprehensive Authorization Coverage Verification
// ============================================================

#[test]
fn test_all_state_changing_entrypoints_require_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let non_admin = Address::generate(&env);
    let new_addr = Address::generate(&env);
    let config = default_fee_config();

    // Each admin-restricted entrypoint invoked with a non-admin caller
    // should return ContractError::Unauthorized.

    let r = client.try_update_fee_config(&non_admin, &config);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "update_fee_config should reject non-admin"
    );

    let r = client.try_set_platform_fee(&non_admin, &300);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "set_platform_fee should reject non-admin"
    );

    let r = client.try_collect_fees(&non_admin);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "collect_fees should reject non-admin"
    );

    let r = client.try_pause_fees(&non_admin);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "pause_fees should reject non-admin"
    );

    let r = client.try_unpause_fees(&non_admin);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "unpause_fees should reject non-admin"
    );

    let r = client.try_transfer_admin(&non_admin, &new_addr);
    assert!(
        matches!(r, Err(Ok(ContractError::Unauthorized))),
        "transfer_admin should reject non-admin"
    );

    // record_fee authenticates its authorized contract caller, not the admin.
    let (_, authorized_caller) =
        invoke_as_authorized_caller(&env, &client, 1000, |market, fees, caller, amount| {
            market.record_fee(fees, caller, amount)
        });
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(
        auths[0].0, authorized_caller,
        "record_fee should auth the authorized contract"
    );
}

#[test]
fn test_all_state_changing_entrypoints_succeed_with_proper_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let config = default_fee_config();

    let hash = fee_config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    assert!(
        client.try_update_fee_config(&admin, &config).is_ok(),
        "Admin should invoke update_fee_config"
    );
    assert!(
        client.try_set_platform_fee(&admin, &300).is_ok(),
        "Admin should invoke set_platform_fee"
    );
    assert!(
        client.try_pause_fees(&admin).is_ok(),
        "Admin should invoke pause_fees"
    );
    assert!(
        client.try_unpause_fees(&admin).is_ok(),
        "Admin should invoke unpause_fees"
    );

    // record_fee requires an authorized contract caller.
    assert!(
        invoke_as_authorized_caller(&env, &client, 200_000_000, |market, fees, caller, amount| {
            market.try_record_fee(fees, caller, amount)
        })
        .0
        .is_ok(),
        "Authorized contract should invoke record_fee"
    );

    // collect_fees needs fees above default threshold (100M stroops)
    assert!(
        client.try_collect_fees(&admin).is_ok(),
        "Admin should invoke collect_fees"
    );

    let another_admin = Address::generate(&env);
    assert!(
        client.try_transfer_admin(&admin, &another_admin).is_ok(),
        "Admin should invoke transfer_admin"
    );
}

#[test]
fn test_get_admin_after_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(FeesContract, ());
    let client = FeesContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

// ============================================================
// Pending Fee Inspection & Commit-Reveal-Apply Lifecycle Tests
// ============================================================

#[test]
fn test_pending_fee_commit_inspection() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // Initially no pending commit
    assert_eq!(client.get_pending_fee_commit(), None);

    // Commit a hash
    let hash = soroban_sdk::BytesN::from_array(&env, &[0x55u8; 32]);
    client.commit_fee_config(&admin, &hash);

    // Public read path returns pending commitment details
    let pending = client.get_pending_fee_commit().expect("pending commit must be present");
    assert_eq!(pending.hash, hash);
    assert_eq!(pending.admin, admin);
    assert_eq!(pending.committed_at, env.ledger().timestamp());
}

#[test]
fn test_update_fee_config_rejects_reveal_before_timelock() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);
    let hash = soroban_sdk::BytesN::from_array(&env, &[0x55u8; 32]);
    client.commit_fee_config(&admin, &hash);

    match client.try_update_fee_config(&admin, &default_fee_config()) {
        Err(Ok(ContractError::FeeRevealTooEarly)) => {}
        other => panic!("Expected FeeRevealTooEarly, got: {:?}", other),
    }
}

#[test]
fn test_pending_fee_config_inspection_and_apply() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    // Initially no pending config
    assert_eq!(client.get_pending_fee_config(), None);

    // Update stages the config without changing the active configuration
    let new_cfg = default_fee_config();
    let original_cfg = client.get_fee_config();
    assert_ne!(new_cfg, original_cfg);
    let hash = fee_config_hash(&env, &new_cfg);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &new_cfg);
    assert_eq!(client.get_fee_config(), original_cfg);

    // Public read path returns in-flight pending configuration
    let pending = client.get_pending_fee_config().expect("pending config must be present");
    assert_eq!(pending.config, new_cfg);
    assert_eq!(pending.admin, admin);
    assert_eq!(pending.proposed_at, env.ledger().timestamp());
    assert_eq!(pending.apply_eta, env.ledger().timestamp() + 86_400);

    // Apply before timelock fails
    let early_res = client.try_apply_fee_update(&admin);
    match early_res {
        Err(Ok(ContractError::FeeApplyTooEarly)) => {}
        other => panic!("Expected FeeApplyTooEarly, got: {:?}", other),
    }

    // Fast-forward past timelock
    env.ledger().with_mut(|l| {
        l.timestamp += 86_401;
    });

    // Apply after timelock succeeds
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config(), new_cfg);
    assert_eq!(client.get_pending_fee_config(), None);
}

#[test]
fn test_cancel_fee_update() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let client = register_and_initialize(&env, &admin);

    let hash = soroban_sdk::BytesN::from_array(&env, &[0x77u8; 32]);
    client.commit_fee_config(&admin, &hash);
    assert!(client.get_pending_fee_commit().is_some());

    client.cancel_fee_update(&admin);
    assert_eq!(client.get_pending_fee_commit(), None);
    assert_eq!(client.get_pending_fee_config(), None);
}
