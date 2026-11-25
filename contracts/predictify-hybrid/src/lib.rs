#![no_std]

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern crate alloc;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Map, String, Symbol, Vec,
};

pub const PERCENTAGE_DENOMINATOR: i128 = 10000;
pub const SYM_ADMIN: &str = "Admin";
pub const ORACLE_FAILURE_PRIMARY_THEN_FALLBACK_REASON: &str = "Primary oracle failed, fallback also failed";
const SYM_ALLOWED_ASSETS: &str = "allowed"; 

#[path = "admin/mod.rs"]
mod admin;
pub mod analytics;
pub mod audit;
pub mod audit_trail;
pub mod monitor;
mod balances;
mod batch_operations;
mod bets;
pub mod circuit_breaker;
mod config;
mod delegation;
mod err;
pub mod errors;
mod force_resolve;
mod event_archive;
pub mod events;
mod extensions;
pub mod gov_registry;
mod fees;
mod gas;
mod governance;
mod markets;
mod monitoring;
mod oracles;
mod reentrancy_guard;
mod oracle_health;
mod reporting;

#[cfg(test)]
mod resolution_event_ordering_tests;
#[cfg(test)]
mod resolution_state_property_tests;

#[cfg(test)]
mod tests {
    mod auth_snap;
}

#[cfg(test)]
#[path = "tests/oracle_validation_tests.rs"]
mod oracle_validation_tests;

#[cfg(test)]
#[path = "tests/market_closed_tests.rs"]
mod market_closed_tests;

#[cfg(test)]
#[path = "tests/recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
mod cross_oracle_staleness_tests;
mod resolution;
mod storage;
mod tokens;

// Domain-specific type modules
mod bet_types;
mod dispute_types;
mod fee_types;
mod governance_types;
mod market_types;
mod oracle_types;
mod query_types;
mod statistics_types;
mod support_types;
pub mod types;

mod upgrade_manager;
mod utils;
mod validation;
mod versioning;
mod voting;
mod voting_delegation;
mod voting_delegation_integration;
mod market_analytics;
mod performance_benchmarks;
mod disputes;
mod edge_cases;
mod graceful_degradation;
mod market_id_generator;
mod metadata_limits;
mod queries;
mod recovery;
mod statistics;
mod rate_limiter;
mod dispute_multisig;
mod event_topic_catalog;
mod storage_tier_audit;
pub mod leaderboard;
mod lists;
mod capabilities;

#[cfg(test)]
mod override_audit_tests;
#[cfg(test)]
mod market_audit_tests;
#[cfg(test)]
mod test_audit_trail;
#[cfg(test)]
mod tests;

mod bandprotocol {
    soroban_sdk::contractimport!(file = "./std_reference.wasm");
}

pub mod timelock;

mod contract;
pub use contract::PredictifyHybrid;

use bets::BetStorage;
use gas::BudgetGuard;
use resolution::ResolutionOutcomeCache;
use storage::BalanceStorage;
use types::{Market, ReflectorAsset};
use events::EventEmitter;
// `CircuitBreaker`, `Error`, `EventEmitter`, `ClaimInfo` and the soroban_sdk
// prelude items are imported/re-exported once below; duplicating them here
// tripped E0252 "defined multiple times".

#[cfg(test)]
mod resolution_invariants;
#[cfg(test)]
mod market_state_matrix_tests;
#[cfg(test)]
mod upgrade_manager_tests;
#[cfg(test)]
mod oracle_lifecycle_events_tests;
mod timelock_tests;
#[cfg(test)]
mod market_leaderboard_tests;

#[cfg(test)]
mod governance_tests;

#[cfg(any())]
mod category_tags_tests;
#[cfg(test)]
mod tie_resolution_tests;
#[cfg(test)]
mod force_resolve_tests;

#[cfg(test)]
mod analytics_snapshot_tests;
#[cfg(test)]
mod betting_invariant_proptest;
#[cfg(test)]
mod property_based_tests;
#[cfg(test)]
mod betting_invariants;
mod analytics_snapshot;

#[cfg(test)]
mod max_participants_tests;

#[cfg(test)]
#[path = "tests/fee_config_commit_reveal_tests.rs"]
mod fee_config_commit_reveal_tests;

#[cfg(test)]
mod admin_cooldown_tests;

// Re-export commonly used items
use admin::{AdminAnalyticsResult, AdminInitializer, AdminManager, AdminPermission, AdminRole};
use crate::Error;
// Backwards-compatible re-export for existing module paths.
pub use crate::Error;
// pub use queries::QueryManager;
pub use audit_trail::{AuditAction, AuditRecord, AuditTrailHead, AuditTrailManager};
pub use types::*;

use crate::config::{
    ConfigManager, DEFAULT_PLATFORM_FEE_PERCENTAGE, MAX_PLATFORM_FEE_PERCENTAGE,
    MIN_PLATFORM_FEE_PERCENTAGE,
};
use crate::gas::GasTracker;
use crate::graceful_degradation::{OracleBackup, OracleHealth};
use crate::market_id_generator::MarketIdGenerator;
use alloc::format;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short,
    Address, BytesN, Env, Map, String, Symbol, Vec,
};

impl From<crate::reentrancy_guard::GuardError> for Error {
    fn from(_err: crate::reentrancy_guard::GuardError) -> Self {
        Error::InvalidState
    }
}

impl From<crate::rate_limiter::RateLimiterError> for Error {
    fn from(err: crate::rate_limiter::RateLimiterError) -> Self {
        match err {
            crate::rate_limiter::RateLimiterError::RateLimitExceeded => Error::RateLimitExceeded,
            crate::rate_limiter::RateLimiterError::ConfigNotFound => Error::ConfigNotFound,
            crate::rate_limiter::RateLimiterError::Unauthorized => Error::Unauthorized,
            _ => Error::RateLimitExceeded,
        }
    }
}

fn resolution_timeout_reached(env: &Env, market: &Market) -> bool {
    let current_time = env.ledger().timestamp();
    current_time >= market.end_time.saturating_add(market.resolution_timeout)
}

fn automatic_oracle_result_unavailable(
    env: &Env,
    config: &OracleConfig,
) -> Result<String, Error> {
    if !config.is_active() {
        return Err(Error::OracleUnavailable);
    }
    Ok(String::from_str(env, "pending"))
}

/// Probe an oracle for an automatic result; returns the raw oracle result string.
fn get_oracle_result(env: &Env, config: &OracleConfig) -> Result<String, Error> {
    automatic_oracle_result_unavailable(env, config)
}
