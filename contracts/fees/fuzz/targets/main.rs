#![no_main]

//! Stateful fuzz target for the fees contract commit-reveal scheme.
//!
//! Corpus actions cover arbitrary commitments and preimages, mismatched reveals,
//! queue cancellation, and ledger-time transitions before and after the timelock.

use fees::{ContractError, FeeConfig, FeesContract, FeesContractClient};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Bytes, BytesN, Env,
};

const NUM_ACTIONS: u8 = 5;
const TIMELOCK_SECONDS: u64 = 86_400;

fn hash_fee_config(env: &Env, config: &FeeConfig) -> BytesN<32> {
    let mut encoded = Bytes::new(env);
    encoded.append(&Bytes::from_slice(
        env,
        &config.platform_fee_percentage.to_be_bytes(),
    ));
    encoded.append(&Bytes::from_slice(env, &config.creation_fee.to_be_bytes()));
    encoded.append(&Bytes::from_slice(env, &config.min_fee_amount.to_be_bytes()));
    encoded.append(&Bytes::from_slice(env, &config.max_fee_amount.to_be_bytes()));
    encoded.append(&Bytes::from_slice(
        env,
        &config.collection_threshold.to_be_bytes(),
    ));
    encoded.append(&Bytes::from_slice(
        env,
        &[if config.fees_enabled { 1u8 } else { 0u8 }],
    ));
    env.crypto().sha256(&encoded).into()
}

fn decode_config(env: &Env, data: &[u8]) -> FeeConfig {
    let platform = i128::from_be_bytes(data[0..16].try_into().unwrap());
    let creation = i128::from_be_bytes(data[16..32].try_into().unwrap());
    let min_fee = i128::from_be_bytes(data[32..48].try_into().unwrap());
    let max_fee = i128::from_be_bytes(data[48..64].try_into().unwrap());
    let threshold = i128::from_be_bytes(data[64..80].try_into().unwrap());

    FeeConfig {
        platform_fee_percentage: platform.rem_euclid(10_001),
        creation_fee: creation.rem_euclid(1_000_000_001),
        min_fee_amount: min_fee.rem_euclid(1_000_000_001),
        max_fee_amount: max_fee.rem_euclid(1_000_000_001),
        collection_threshold: threshold.rem_euclid(1_000_000_001),
        fees_enabled: data[80] & 1 != 0,
    }
}

fn assert_allowed_error(error: &ContractError) {
    assert!(
        matches!(
            error,
            ContractError::Unauthorized
                | ContractError::AdminNotSet
                | ContractError::InvalidInput
                | ContractError::InvalidState
                | ContractError::Overflow
                | ContractError::FeesPaused
                | ContractError::FeeConfigNotFound
                | ContractError::FeePercentageTooHigh
                | ContractError::BelowCollectionThreshold
                | ContractError::NoPendingFeeCommit
                | ContractError::FeePreimageMismatch
                | ContractError::FeeRevealTooEarly
                | ContractError::NoQueuedFeeUpdate
        ),
        "unexpected fees contract error: {error:?}"
    );
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 82 {
        return;
    }

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let contract_id = env.register_contract(None, FeesContract);
    let client = FeesContractClient::new(&env, &contract_id);

    let _ = client.try_initialize(&admin);
    let mut idx = 0;

    while idx < data.len() {
        let action = data[idx] % NUM_ACTIONS;
        idx += 1;

        match action {
            // Commit an arbitrary preimage hash.
            0 => {
                if idx + 33 > data.len() {
                    break;
                }
                let caller = if data[idx] & 1 == 0 {
                    admin.clone()
                } else {
                    non_admin.clone()
                };
                let preimage = &data[idx + 1..idx + 33];
                let commitment = env.crypto().sha256(&Bytes::from_slice(&env, preimage));
                idx += 33;

                let result = client.try_commit_fee_config(&caller, &commitment);
                if let Err(Ok(error)) = result {
                    assert_allowed_error(&error);
                }
            }
            // Reveal a matching or mismatched fee configuration.
            1 => {
                if idx + 82 > data.len() {
                    break;
                }
                let caller = if data[idx] & 1 == 0 {
                    admin.clone()
                } else {
                    non_admin.clone()
                };
                let mismatch = data[idx + 1] & 1 != 0;
                let mut config = decode_config(&env, &data[idx + 2..idx + 82]);
                if mismatch {
                    config.creation_fee = config.creation_fee.wrapping_add(1);
                }
                idx += 82;

                // Seed a commitment for mismatch coverage when needed.
                let commitment = if mismatch {
                    let mut different = config.clone();
                    different.platform_fee_percentage =
                        (different.platform_fee_percentage + 1).rem_euclid(10_001);
                    hash_fee_config(&env, &different)
                } else {
                    hash_fee_config(&env, &config)
                };
                let _ = client.try_commit_fee_config(&admin, &commitment);

                let result = client.try_reveal_fee_config(&caller, &config);
                if let Err(Ok(error)) = result {
                    assert_allowed_error(&error);
                }
            }
            // Apply a queued update before or after the timelock.
            2 => {
                if idx + 9 > data.len() {
                    break;
                }
                let caller = if data[idx] & 1 == 0 {
                    admin.clone()
                } else {
                    non_admin.clone()
                };
                let delta = u64::from_be_bytes(data[idx + 1..idx + 9].try_into().unwrap());
                idx += 9;

                let current = env.ledger().timestamp();
                env.ledger().set(LedgerInfo {
                    timestamp: current.saturating_add(delta % (TIMELOCK_SECONDS * 2)),
                    protocol_version: env.ledger().protocol_version(),
                    sequence_number: env.ledger().sequence() + 1,
                    network_id: env.ledger().network_id(),
                    base_reserve: env.ledger().base_reserve(),
                    min_temp_entry_ttl: env.ledger().min_temp_entry_ttl(),
                    min_persistent_entry_ttl: env.ledger().min_persistent_entry_ttl(),
                    max_entry_ttl: env.ledger().max_entry_ttl(),
                });

                let result = client.try_apply_fee_update(&caller);
                if let Err(Ok(error)) = result {
                    assert_allowed_error(&error);
                }
            }
            // Cancel a queued update.
            3 => {
                let caller = if idx < data.len() && data[idx] & 1 == 0 {
                    admin.clone()
                } else {
                    non_admin.clone()
                };
                if idx < data.len() {
                    idx += 1;
                }
                let result = client.try_cancel_fee_update(&caller);
                if let Err(Ok(error)) = result {
                    assert_allowed_error(&error);
                }
            }
            // Advance ledger independently to stress timelock boundaries.
            4 => {
                if idx + 8 > data.len() {
                    break;
                }
                let delta = u64::from_be_bytes(data[idx..idx + 8].try_into().unwrap());
                idx += 8;
                let current = env.ledger().timestamp();
                env.ledger().set(LedgerInfo {
                    timestamp: current.saturating_add(delta % (TIMELOCK_SECONDS * 4)),
                    protocol_version: env.ledger().protocol_version(),
                    sequence_number: env.ledger().sequence() + 1,
                    network_id: env.ledger().network_id(),
                    base_reserve: env.ledger().base_reserve(),
                    min_temp_entry_ttl: env.ledger().min_temp_entry_ttl(),
                    min_persistent_entry_ttl: env.ledger().min_persistent_entry_ttl(),
                    max_entry_ttl: env.ledger().max_entry_ttl(),
                });
            }
            _ => unreachable!(),
        }
    }

    let _ = client.get_fee_config();
    let _ = client.get_collected_fees();
    let _ = client.get_withdrawal_schedule();
});
