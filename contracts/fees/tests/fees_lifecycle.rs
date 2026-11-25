#![cfg(test)]

//! Integration tests for the full commit → reveal (update) → apply fee lifecycle.
//!
//! Each test exercises exactly one scenario of the three-step flow so that a
//! regression in any single step is easy to pinpoint.  Timestamp manipulation
//! is done via `env.ledger().set_timestamp()` / `env.ledger().with_mut(...)` to
//! simulate timelock expiry without real time passing.

use fees::{ContractError, FeeConfig, FeesContract, FeesContractClient, MAX_FEE_AMOUNT};
use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Address, Env};

// ============================================================
// Shared helpers
// ============================================================

/// Registers the fees contract and initialises it with `admin`.
/// `mock_all_auths` is activated so every subsequent `require_auth` call passes.
fn setup(env: &Env) -> (FeesContractClient, Address) {
    let id = env.register(FeesContract, ());
    let client = FeesContractClient::new(env, &id);
    env.mock_all_auths();
    let admin = Address::generate(env);
    client.initialize(&admin);
    // Drain the initialise auth snapshot so it doesn't pollute later assertions.
    let _ = env.auths();
    (client, admin)
}

/// Returns a valid non-default fee configuration for use as the proposed update.
fn proposed_config() -> FeeConfig {
    FeeConfig {
        platform_fee_percentage: 500, // 5 %
        creation_fee: 200,
        min_fee_amount: 10,
        max_fee_amount: 5_000_000_000, // 5 000 XLM — within the 10 000 XLM hard cap
        collection_threshold: 50_000,
        fees_enabled: true,
    }
}

/// Computes the sha256 hash of `config`'s XDR encoding, matching what
/// `update_fee_config` verifies.
fn config_hash(env: &Env, config: &FeeConfig) -> soroban_sdk::BytesN<32> {
    env.crypto().sha256(&config.to_xdr(env))
}

// ============================================================
// 1. Happy path — commit → reveal → apply
// ============================================================

/// The canonical three-step flow must leave the active fee config equal to the
/// proposed one after `apply_fee_update` succeeds.
#[test]
fn test_full_lifecycle_happy_path() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let config = proposed_config();
    let original = client.get_fee_config();
    assert_ne!(config, original, "test config must differ from the default");

    // ── Step 1: commit ──────────────────────────────────────────────────────
    let hash = config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);

    // Commitment is immediately visible via the read-only accessor.
    let commit = client
        .get_pending_fee_commit()
        .expect("pending commit must be stored after commit_fee_config");
    assert_eq!(commit.hash, hash);
    assert_eq!(commit.admin, admin);
    assert_eq!(commit.committed_at, env.ledger().timestamp());

    // Active config is still unchanged.
    assert_eq!(client.get_fee_config(), original);

    // ── Step 2: reveal (update_fee_config) ──────────────────────────────────
    let commit_time = env.ledger().timestamp();
    client.update_fee_config(&admin, &config);

    // The commit record is consumed; the queued config is now visible.
    assert_eq!(client.get_pending_fee_commit(), None);
    let queued = client
        .get_pending_fee_config()
        .expect("pending config must be stored after update_fee_config");
    assert_eq!(queued.config, config);
    assert_eq!(queued.admin, admin);
    assert_eq!(queued.proposed_at, commit_time);
    // apply_eta = proposed_at + 86 400 s (DEFAULT_TIMELOCK_SECONDS)
    assert_eq!(queued.apply_eta, commit_time + 86_400);

    // Active config is still the original — the timelock has not expired.
    assert_eq!(client.get_fee_config(), original);

    // ── Step 3: advance time past the timelock, then apply ──────────────────
    env.ledger().with_mut(|l| {
        l.timestamp = queued.apply_eta + 1;
    });

    client.apply_fee_update(&admin);

    // The active config must now equal the proposed one.
    assert_eq!(client.get_fee_config(), config);
    // The pending queue entry must have been cleared.
    assert_eq!(client.get_pending_fee_config(), None);
}

// ============================================================
// 2. Timelock boundary — apply_fee_update rejected before ETA
// ============================================================

/// Attempting to apply exactly one second *before* the ETA must return
/// `FeeRevealTooEarly`.  Applying at the ETA timestamp (not strictly after)
/// must also be rejected because the contract uses `now < apply_eta`.
#[test]
fn test_apply_rejected_before_timelock_expires() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let config = proposed_config();
    let hash = config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);

    let queued = client.get_pending_fee_config().unwrap();

    // One second before the ETA → must be rejected.
    env.ledger().with_mut(|l| {
        l.timestamp = queued.apply_eta - 1;
    });
    let result = client.try_apply_fee_update(&admin);
    assert!(
        matches!(result, Err(Ok(ContractError::FeeRevealTooEarly))),
        "apply must be rejected one second before ETA, got: {result:?}"
    );

    // Exactly at ETA → `now < apply_eta` is false, so it must succeed.
    env.ledger().with_mut(|l| {
        l.timestamp = queued.apply_eta;
    });
    let result = client.try_apply_fee_update(&admin);
    assert!(
        result.is_ok(),
        "apply must succeed exactly at the ETA timestamp, got: {result:?}"
    );
    assert_eq!(client.get_fee_config(), config);
}

// ============================================================
// 3. Reveal rejected without a prior commit
// ============================================================

/// Calling `update_fee_config` without a preceding `commit_fee_config` must
/// return `NoPendingFeeCommit`.
#[test]
fn test_reveal_requires_prior_commit() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let config = proposed_config();
    let result = client.try_update_fee_config(&admin, &config);
    assert!(
        matches!(result, Err(Ok(ContractError::NoPendingFeeCommit))),
        "update_fee_config must require a prior commit, got: {result:?}"
    );
}

// ============================================================
// 4. Reveal rejected when config does not match the committed hash
// ============================================================

/// If the revealed `FeeConfig` hashes to something other than the stored
/// commitment, `update_fee_config` must return `CommitHashMismatch`.
#[test]
fn test_reveal_rejected_on_hash_mismatch() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let committed_config = proposed_config();
    let hash = config_hash(&env, &committed_config);
    client.commit_fee_config(&admin, &hash);

    // Reveal a *different* config — must be rejected.
    let different_config = FeeConfig {
        platform_fee_percentage: 100,
        ..proposed_config()
    };
    let result = client.try_update_fee_config(&admin, &different_config);
    assert!(
        matches!(result, Err(Ok(ContractError::CommitHashMismatch))),
        "update_fee_config must reject a config that does not match the hash, got: {result:?}"
    );

    // Commitment must still be intact so the admin can retry.
    assert!(
        client.get_pending_fee_commit().is_some(),
        "commitment must survive a failed reveal"
    );
}

// ============================================================
// 5. Active config unchanged between reveal and apply
// ============================================================

/// After a successful reveal the active fee config must not change until
/// `apply_fee_update` is called.
#[test]
fn test_active_config_unchanged_between_reveal_and_apply() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let original = client.get_fee_config();
    let config = proposed_config();
    let hash = config_hash(&env, &config);

    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);

    // Active config must still be the original one.
    assert_eq!(
        client.get_fee_config(),
        original,
        "active config must not change before apply"
    );

    // Advance past the timelock and apply.
    env.ledger().with_mut(|l| {
        l.timestamp += 86_401;
    });
    client.apply_fee_update(&admin);

    // Now it must match the proposed config.
    assert_eq!(
        client.get_fee_config(),
        config,
        "active config must equal the proposed config after apply"
    );
}

// ============================================================
// 6. Cancel mid-lifecycle
// ============================================================

/// Cancelling after `commit_fee_config` but before `update_fee_config` must
/// clear the commitment.  A fresh cycle can then be started from scratch.
#[test]
fn test_cancel_after_commit_allows_fresh_cycle() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let config = proposed_config();
    let hash = config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    assert!(client.get_pending_fee_commit().is_some());

    client.cancel_fee_update(&admin);
    assert_eq!(client.get_pending_fee_commit(), None);
    assert_eq!(client.get_pending_fee_config(), None);

    // A fresh cycle must succeed after cancellation.
    let new_config = FeeConfig {
        platform_fee_percentage: 300,
        ..proposed_config()
    };
    let new_hash = config_hash(&env, &new_config);
    client.commit_fee_config(&admin, &new_hash);
    client.update_fee_config(&admin, &new_config);
    env.ledger().with_mut(|l| {
        l.timestamp += 86_401;
    });
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config().platform_fee_percentage, 300);
}

/// Cancelling after `update_fee_config` (queued state) must clear both the
/// commitment and the queued config.
#[test]
fn test_cancel_after_reveal_clears_queue() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let config = proposed_config();
    let hash = config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);
    assert!(client.get_pending_fee_config().is_some());

    client.cancel_fee_update(&admin);
    assert_eq!(client.get_pending_fee_commit(), None);
    assert_eq!(client.get_pending_fee_config(), None);

    // Active config must remain the original.
    let original_pct = 200; // default platform_fee_percentage from initialize
    assert_eq!(
        client.get_fee_config().platform_fee_percentage,
        original_pct
    );
}

// ============================================================
// 7. apply_fee_update fails when no queued config exists
// ============================================================

/// Calling `apply_fee_update` with nothing queued must return
/// `NoPendingFeeCommit` (the error the contract reuses for "no queued config").
#[test]
fn test_apply_with_no_queued_config_returns_error() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let result = client.try_apply_fee_update(&admin);
    assert!(
        matches!(result, Err(Ok(ContractError::NoPendingFeeCommit))),
        "apply_fee_update with nothing queued must return NoPendingFeeCommit, got: {result:?}"
    );
}

// ============================================================
// 8. Second consecutive lifecycle replaces the first applied config
// ============================================================

/// Running two full cycles back-to-back must result in the second proposed
/// config being active after the second apply.
#[test]
fn test_two_consecutive_lifecycles() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    // ── Cycle 1 ─────────────────────────────────────────────────────────────
    let config_a = FeeConfig {
        platform_fee_percentage: 300,
        ..proposed_config()
    };
    let hash_a = config_hash(&env, &config_a);
    client.commit_fee_config(&admin, &hash_a);
    client.update_fee_config(&admin, &config_a);
    env.ledger().with_mut(|l| {
        l.timestamp += 86_401;
    });
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config().platform_fee_percentage, 300);

    // ── Cycle 2 ─────────────────────────────────────────────────────────────
    let config_b = FeeConfig {
        platform_fee_percentage: 750,
        ..proposed_config()
    };
    let hash_b = config_hash(&env, &config_b);
    client.commit_fee_config(&admin, &hash_b);
    client.update_fee_config(&admin, &config_b);
    env.ledger().with_mut(|l| {
        l.timestamp += 86_401;
    });
    client.apply_fee_update(&admin);
    assert_eq!(client.get_fee_config().platform_fee_percentage, 750);
    assert_eq!(client.get_pending_fee_config(), None);
}

// ============================================================
// 9. Timelock window calculation correctness
// ============================================================

/// Verifies that `apply_eta` equals `proposed_at + 86_400` (DEFAULT_TIMELOCK_SECONDS)
/// regardless of what the ledger timestamp was at commit time.
#[test]
fn test_timelock_window_calculation() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    // Advance to an arbitrary non-zero timestamp before the cycle starts.
    let start_ts: u64 = 1_000_000;
    env.ledger().with_mut(|l| {
        l.timestamp = start_ts;
    });

    let config = proposed_config();
    let hash = config_hash(&env, &config);
    client.commit_fee_config(&admin, &hash);
    client.update_fee_config(&admin, &config);

    let queued = client.get_pending_fee_config().unwrap();
    assert_eq!(queued.proposed_at, start_ts);
    assert_eq!(queued.apply_eta, start_ts + 86_400);

    // One second before ETA → rejected.
    env.ledger().with_mut(|l| {
        l.timestamp = queued.apply_eta - 1;
    });
    assert!(matches!(
        client.try_apply_fee_update(&admin),
        Err(Ok(ContractError::FeeRevealTooEarly))
    ));

    // Exactly at ETA → accepted.
    env.ledger().with_mut(|l| {
        l.timestamp = queued.apply_eta;
    });
    assert!(client.try_apply_fee_update(&admin).is_ok());
}

// ============================================================
// 10. Validation is enforced during reveal, not just apply
// ============================================================

/// A config with a platform_fee_percentage above 10 000 bps must be rejected
/// at `update_fee_config` time, before any queued state is written.
#[test]
fn test_invalid_config_rejected_at_reveal() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let invalid_config = FeeConfig {
        platform_fee_percentage: 20_000, // exceeds MAX_FEE_PERCENTAGE
        ..proposed_config()
    };

    // Commit the hash of the invalid config.
    let hash = config_hash(&env, &invalid_config);
    client.commit_fee_config(&admin, &hash);

    // Reveal must be rejected at validation.
    let result = client.try_update_fee_config(&admin, &invalid_config);
    assert!(
        matches!(result, Err(Ok(ContractError::FeePercentageTooHigh))),
        "reveal must reject an invalid config, got: {result:?}"
    );

    // No queued config must have been written.
    assert_eq!(
        client.get_pending_fee_config(),
        None,
        "pending config must remain absent after a failed reveal"
    );
}

/// A config with max_fee_amount exceeding the hard protocol cap
/// (MAX_FEE_AMOUNT = 10 000 XLM) must be rejected at reveal time.
#[test]
fn test_max_fee_amount_cap_enforced_at_reveal() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    let invalid_config = FeeConfig {
        max_fee_amount: MAX_FEE_AMOUNT + 1,
        ..proposed_config()
    };

    let hash = config_hash(&env, &invalid_config);
    client.commit_fee_config(&admin, &hash);

    let result = client.try_update_fee_config(&admin, &invalid_config);
    assert!(
        matches!(result, Err(Ok(ContractError::InvalidInput))),
        "reveal must reject a max_fee_amount above the protocol cap, got: {result:?}"
    );
    assert_eq!(client.get_pending_fee_config(), None);
}
