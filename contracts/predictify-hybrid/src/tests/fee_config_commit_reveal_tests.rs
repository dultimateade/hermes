#![cfg(test)]

use crate::fees::{FeeConfig, FeeConfigManager, FeeManager, PendingFeeCommit, PendingFeeConfig};
use crate::err::Error;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, BytesN, Env};

fn setup_env() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    (env, admin)
}

fn sample_fee_config() -> FeeConfig {
    FeeConfig {
        platform_fee_percentage: 500, // 5.00%
        creation_fee: 10_000,
        min_fee_amount: 100,
        max_fee_amount: 1_000_000,
        collection_threshold: 50_000,
        fees_enabled: true,
    }
}

#[test]
fn test_get_pending_fee_commit_empty_initially() {
    let (env, _admin) = setup_env();
    let pending = FeeManager::get_pending_fee_commit(&env);
    assert_eq!(pending, None);
}

#[test]
fn test_commit_fee_config_and_inspect_pending_commit() {
    let (env, admin) = setup_env();
    let hash = BytesN::from_array(&env, &[0x42u8; 32]);

    FeeManager::commit_fee_config(&env, admin.clone(), hash.clone()).expect("commit should succeed");

    let pending = FeeManager::get_pending_fee_commit(&env).expect("pending commit should be present");
    assert_eq!(pending.hash, hash);
    assert_eq!(pending.admin, admin);
    assert_eq!(pending.committed_at, env.ledger().timestamp());
}

#[test]
fn test_get_pending_fee_config_empty_initially() {
    let (env, _admin) = setup_env();
    let pending = FeeManager::get_pending_fee_config(&env);
    assert_eq!(pending, None);
}

#[test]
fn test_reveal_fee_config_and_inspect_pending_config() {
    let (env, admin) = setup_env();
    let hash = BytesN::from_array(&env, &[0xab; 32]);
    FeeManager::commit_fee_config(&env, admin.clone(), hash).expect("commit should succeed");

    let new_cfg = sample_fee_config();
    let revealed = FeeManager::update_fee_config(&env, admin.clone(), new_cfg.clone())
        .expect("update_fee_config should succeed");
    assert_eq!(revealed, new_cfg);

    // Commit should be consumed
    assert_eq!(FeeManager::get_pending_fee_commit(&env), None);

    // Pending queued config must now be inspectable by governance tooling
    let pending = FeeManager::get_pending_fee_config(&env).expect("pending config must be present");
    assert_eq!(pending.config, new_cfg);
    assert_eq!(pending.admin, admin);
    assert_eq!(pending.proposed_at, env.ledger().timestamp());
    assert_eq!(
        pending.apply_eta,
        env.ledger().timestamp() + FeeManager::DEFAULT_TIMELOCK_SECONDS
    );
}

#[test]
fn test_apply_fee_update_timelock_enforcement() {
    let (env, admin) = setup_env();
    let new_cfg = sample_fee_config();

    FeeManager::update_fee_config(&env, admin.clone(), new_cfg.clone())
        .expect("update_fee_config should succeed");

    // Attempting to apply immediately must fail with FeeRevealTooEarly
    let early_result = FeeManager::apply_fee_update(&env, admin.clone());
    assert_eq!(early_result, Err(Error::FeeRevealTooEarly));

    // Fast-forward ledger time past timelock
    env.ledger().with_mut(|l| {
        l.timestamp += FeeManager::DEFAULT_TIMELOCK_SECONDS + 1;
    });

    // Applying after timelock must succeed
    FeeManager::apply_fee_update(&env, admin.clone()).expect("apply_fee_update should succeed");

    // Active fee config is now updated
    let active_cfg = FeeConfigManager::get_fee_config(&env).expect("get_fee_config should succeed");
    assert_eq!(active_cfg, new_cfg);

    // In-flight pending config is cleared
    assert_eq!(FeeManager::get_pending_fee_config(&env), None);
}

#[test]
fn test_cancel_fee_update_clears_pending() {
    let (env, admin) = setup_env();
    let new_cfg = sample_fee_config();

    FeeManager::update_fee_config(&env, admin.clone(), new_cfg)
        .expect("update_fee_config should succeed");
    assert!(FeeManager::get_pending_fee_config(&env).is_some());

    FeeManager::cancel_fee_update(&env, admin).expect("cancel_fee_update should succeed");
    assert_eq!(FeeManager::get_pending_fee_config(&env), None);
    assert_eq!(FeeManager::get_pending_fee_commit(&env), None);
}

#[test]
fn test_cancel_nothing_queued_fails() {
    let (env, admin) = setup_env();
    let result = FeeManager::cancel_fee_update(&env, admin);
    assert_eq!(result, Err(Error::NoPendingFeeCommit));
}

#[test]
fn test_invalid_fee_config_rejected() {
    let (env, admin) = setup_env();
    let mut bad_cfg = sample_fee_config();
    bad_cfg.platform_fee_percentage = 20_000; // Exceeds 10_000 max

    let result = FeeManager::update_fee_config(&env, admin, bad_cfg);
    assert_eq!(result, Err(Error::InvalidFeeConfig));
}

#[test]
fn test_withdrawal_schedule_initial_and_failed_status() {
    let (env, admin) = setup_env();
    let market_id = soroban_sdk::symbol_short!("mkt1");

    // 1. Initial withdrawal schedule should be Ready
    let initial_schedule = FeeManager::get_withdrawal_schedule(&env);
    assert_eq!(initial_schedule.status, crate::fees::FeeWithdrawalStatus::Ready);

    // 2. Attempting collect_fees on a non-existent / unresolved market fails
    let result = FeeManager::collect_fees(&env, admin, market_id);
    assert_eq!(result, Err(Error::MarketNotFound));

    // 3. Status is preserved or set to Failed on unresolved markets
    let schedule = FeeManager::get_withdrawal_schedule(&env);
    assert!(
        schedule.status == crate::fees::FeeWithdrawalStatus::Ready
            || schedule.status == crate::fees::FeeWithdrawalStatus::Failed
    );
}
