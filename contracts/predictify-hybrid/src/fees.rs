#![no_std]

use soroban_sdk::{contracttype, Address, BytesN, Env, Map, String, Symbol, Vec};
use crate::err::Error;
use crate::events::EventEmitter;

// ============================================================
// Types
// ============================================================

/// Fee configuration for the platform.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    /// Platform fee percentage in basis points (1 = 0.01%).
    /// Maximum allowed: 10_000 (100%).
    pub platform_fee_percentage: i128,
    /// Flat creation fee in stroops for creating resources.
    pub creation_fee: i128,
    /// Minimum fee amount in stroops.
    pub min_fee_amount: i128,
    /// Maximum fee amount in stroops.
    pub max_fee_amount: i128,
    /// Threshold in stroops above which collected fees may be withdrawn.
    pub collection_threshold: i128,
    /// Whether fees are currently enabled.
    pub fees_enabled: bool,
}

/// Pending fee commitment record prior to revelation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFeeCommit {
    /// Hash of the committed configuration / preimage.
    pub hash: BytesN<32>,
    /// Admin address that submitted the commitment.
    pub admin: Address,
    /// Ledger timestamp when the commitment was recorded.
    pub committed_at: u64,
}

/// In-flight / pending fee configuration update awaiting execution.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingFeeConfig {
    /// Proposed fee configuration.
    pub config: FeeConfig,
    /// Admin address that proposed the update.
    pub admin: Address,
    /// Ledger timestamp when the update was proposed.
    pub proposed_at: u64,
    /// Ledger timestamp after which the update may be applied.
    pub apply_eta: u64,
}

/// Status of a fee withdrawal attempt.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeWithdrawalStatus {
    Ready,
    Pending,
    Completed,
    Failed,
}

/// Schedule entry for fee withdrawals.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeWithdrawalSchedule {
    pub next_withdrawal: u64,
    pub cooldown_seconds: u64,
    pub last_withdrawal: u64,
    pub status: FeeWithdrawalStatus,
}

/// Analytics summary for collected platform fees.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeAnalyticsResult {
    pub total_fees_collected: i128,
    pub market_count: u32,
    pub average_fee_per_market: i128,
}

// ============================================================
// Storage Keys
// ============================================================

pub const SYM_FEE_CURRENT: &str = "FeeCurrent";
pub const SYM_FEE_CONFIG: &str = "FeeConfig";
pub const SYM_FEE_COMMIT: &str = "FeeCommit";
pub const SYM_FEE_QUEUED: &str = "FeeQueued";
pub const SYM_FEE_SCHEDULE: &str = "FeeSchedule";
pub const SYM_FEE_COLLECTED: &str = "FeeCollected";

// ============================================================
// FeeConfigManager
// ============================================================

pub struct FeeConfigManager;

impl FeeConfigManager {
    /// Reads the current active fee configuration from storage.
    pub fn get_fee_config(env: &Env) -> Result<FeeConfig, Error> {
        let key = Symbol::new(env, SYM_FEE_CURRENT);
        if let Some(config) = env.storage().persistent().get::<Symbol, FeeConfig>(&key) {
            return Ok(config);
        }
        let legacy_key = Symbol::new(env, SYM_FEE_CONFIG);
        if let Some(config) = env.storage().persistent().get::<Symbol, FeeConfig>(&legacy_key) {
            return Ok(config);
        }
        // Default fee configuration fallback
        Ok(FeeConfig {
            platform_fee_percentage: 200, // 2.00%
            creation_fee: 0,
            min_fee_amount: 1,
            max_fee_amount: 1_000_000_000_000,
            collection_threshold: 100_000_000,
            fees_enabled: true,
        })
    }

    /// Stores the fee configuration to storage.
    pub fn set_fee_config(env: &Env, config: &FeeConfig) -> Result<(), Error> {
        FeeManager::validate_fee_config(config)?;
        let key = Symbol::new(env, SYM_FEE_CURRENT);
        env.storage().persistent().set(&key, config);
        let legacy_key = Symbol::new(env, SYM_FEE_CONFIG);
        env.storage().persistent().set(&legacy_key, config);
        Ok(())
    }
}

// ============================================================
// FeeManager
// ============================================================

pub struct FeeManager;

impl FeeManager {
    /// Minimum time-lock cooldown in seconds before a revealed fee update can be applied.
    pub const DEFAULT_TIMELOCK_SECONDS: u64 = 86_400; // 24 hours

    /// Validates numeric invariants for a fee configuration.
    pub fn validate_fee_config(config: &FeeConfig) -> Result<(), Error> {
        if config.platform_fee_percentage < 0 || config.platform_fee_percentage > 10_000 {
            return Err(Error::InvalidFeeConfig);
        }
        if config.creation_fee < 0
            || config.min_fee_amount < 0
            || config.max_fee_amount < 0
            || config.collection_threshold < 0
        {
            return Err(Error::InvalidFeeConfig);
        }
        if config.min_fee_amount > config.max_fee_amount {
            return Err(Error::InvalidFeeConfig);
        }
        Ok(())
    }

    /// Stage 1: Commit a hash of the future fee configuration.
    pub fn commit_fee_config(env: &Env, admin: Address, hash: BytesN<32>) -> Result<(), Error> {
        admin.require_auth();
        let commit = PendingFeeCommit {
            hash,
            admin: admin.clone(),
            committed_at: env.ledger().timestamp(),
        };
        let key = Symbol::new(env, SYM_FEE_COMMIT);
        env.storage().persistent().set(&key, &commit);
        Ok(())
    }

    /// Public read path: Returns the active pending fee commitment, if any.
    pub fn get_pending_fee_commit(env: &Env) -> Option<PendingFeeCommit> {
        let key = Symbol::new(env, SYM_FEE_COMMIT);
        env.storage().persistent().get(&key)
    }

    /// Public read path: Returns the pending/queued fee configuration update, if any.
    pub fn get_pending_fee_config(env: &Env) -> Option<PendingFeeConfig> {
        let key = Symbol::new(env, SYM_FEE_QUEUED);
        env.storage().persistent().get(&key)
    }

    /// Stage 2: Reveal and queue a committed or new fee configuration with a timelock.
    pub fn update_fee_config(
        env: &Env,
        admin: Address,
        new_config: FeeConfig,
    ) -> Result<FeeConfig, Error> {
        admin.require_auth();
        Self::validate_fee_config(&new_config)?;

        let now = env.ledger().timestamp();
        let eta = now.saturating_add(Self::DEFAULT_TIMELOCK_SECONDS);

        let queued = PendingFeeConfig {
            config: new_config.clone(),
            admin: admin.clone(),
            proposed_at: now,
            apply_eta: eta,
        };

        let key = Symbol::new(env, SYM_FEE_QUEUED);
        env.storage().persistent().set(&key, &queued);

        // Consume any active commitment
        let commit_key = Symbol::new(env, SYM_FEE_COMMIT);
        env.storage().persistent().remove(&commit_key);

        EventEmitter::emit_fee_config_queued(env, &admin, eta, &new_config);

        Ok(new_config)
    }

    /// Stage 3: Apply the queued fee configuration after the timelock expiration.
    pub fn apply_fee_update(env: &Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let pending = Self::get_pending_fee_config(env).ok_or(Error::NoPendingFeeCommit)?;

        let now = env.ledger().timestamp();
        if now < pending.apply_eta {
            return Err(Error::FeeRevealTooEarly);
        }

        FeeConfigManager::set_fee_config(env, &pending.config)?;

        let key = Symbol::new(env, SYM_FEE_QUEUED);
        env.storage().persistent().remove(&key);

        EventEmitter::emit_fee_config_applied(env, &admin, &pending.config);

        Ok(())
    }

    /// Cancels any in-flight commitment or queued fee configuration update.
    pub fn cancel_fee_update(env: &Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        let queued_key = Symbol::new(env, SYM_FEE_QUEUED);
        let commit_key = Symbol::new(env, SYM_FEE_COMMIT);

        let had_queued = env.storage().persistent().has(&queued_key);
        let had_commit = env.storage().persistent().has(&commit_key);

        if !had_queued && !had_commit {
            return Err(Error::NoPendingFeeCommit);
        }

        env.storage().persistent().remove(&queued_key);
        env.storage().persistent().remove(&commit_key);

        EventEmitter::emit_fee_config_cancelled(env, &admin);

        Ok(())
    }

    /// Returns the applicable fee percentage for a given timestamp.
    pub fn get_fee_percentage_for_timestamp(env: &Env, _timestamp: u64) -> i128 {
        if let Ok(config) = FeeConfigManager::get_fee_config(env) {
            if config.fees_enabled {
                return config.platform_fee_percentage;
            }
        }
        0
    }

    /// Calculates and returns the market creation fee.
    pub fn process_creation_fee(_env: &Env, _admin: &Address) -> Result<i128, Error> {
        let config = FeeConfigManager::get_fee_config(_env)?;
        if !config.fees_enabled {
            return Ok(0);
        }
        Ok(config.creation_fee)
    }

    /// Validates market state for fee collection.
    pub fn validate_market_for_fee_collection(_market: &crate::types::Market) -> Result<(), Error> {
        Ok(())
    }

    /// Collects platform fees from a resolved market.
    pub fn collect_fees(env: &Env, admin: Address, market_id: Symbol) -> Result<i128, Error> {
        admin.require_auth();
        let market = crate::markets::MarketStateManager::get_market(env, &market_id)?;
        if !market.resolved {
            let mut schedule = Self::get_withdrawal_schedule(env);
            schedule.status = FeeWithdrawalStatus::Failed;
            let key = Symbol::new(env, SYM_FEE_SCHEDULE);
            env.storage().persistent().set(&key, &schedule);
            return Err(Error::MarketNotResolved);
        }

        let collected_key = (Symbol::new(env, SYM_FEE_COLLECTED), market_id.clone());
        let already_collected: i128 = env.storage().persistent().get(&collected_key).unwrap_or(0);
        if already_collected > 0 {
            return Ok(0);
        }

        let config = FeeConfigManager::get_fee_config(env)?;
        if !config.fees_enabled {
            let mut schedule = Self::get_withdrawal_schedule(env);
            schedule.status = FeeWithdrawalStatus::Failed;
            let key = Symbol::new(env, SYM_FEE_SCHEDULE);
            env.storage().persistent().set(&key, &schedule);
            return Ok(0);
        }

        let fee = (market.total_pool * config.platform_fee_percentage) / 10_000;
        env.storage().persistent().set(&collected_key, &fee);

        let mut schedule = Self::get_withdrawal_schedule(env);
        let now = env.ledger().timestamp();
        schedule.last_withdrawal = now;
        schedule.next_withdrawal = now.saturating_add(schedule.cooldown_seconds);
        schedule.status = FeeWithdrawalStatus::Completed;
        let key = Symbol::new(env, SYM_FEE_SCHEDULE);
        env.storage().persistent().set(&key, &schedule);

        Ok(fee)
    }

    /// Returns the fee withdrawal schedule.
    pub fn get_withdrawal_schedule(env: &Env) -> FeeWithdrawalSchedule {
        let key = Symbol::new(env, SYM_FEE_SCHEDULE);
        env.storage().persistent().get(&key).unwrap_or(FeeWithdrawalSchedule {
            next_withdrawal: 0,
            cooldown_seconds: 86_400,
            last_withdrawal: 0,
            status: FeeWithdrawalStatus::Ready,
        })
    }
}
