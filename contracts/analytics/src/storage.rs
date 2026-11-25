//! Storage keys and limits for the Analytics contract.

use soroban_sdk::{contracttype, Symbol};

/// Default maximum number of data points stored per metric stream.
pub const DEFAULT_MAX_CAPACITY: u32 = 10_000;

/// Maximum number of time buckets permitted in a single query response.
pub const MAX_BUCKETS_PER_QUERY: u64 = 2_000;

/// Minimum allowed window duration in seconds (1 second).
pub const MIN_WINDOW_SECONDS: u64 = 1;

/// Maximum allowed window duration in seconds (1 year).
pub const MAX_WINDOW_SECONDS: u64 = 31_536_000;

/// Storage keys used by the Analytics contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Whether the contract has been initialized (`bool`).
    Initialized,
    /// The current admin address (`Address`).
    Admin,
    /// Whether analytics recording and queries are paused (`bool`).
    Paused,
    /// Configured max data points capacity per metric (`u32`).
    MaxCapacity,
    /// Persistent sequence of recorded data points for a global metric (`Vec<DataPoint>`).
    MetricPoints(Symbol),
    /// Persistent sequence of recorded data points for a market-specific metric (`Vec<DataPoint>`).
    MarketMetricPoints(Symbol, Symbol),
    /// Running summary statistics for a global metric (`MetricSummary`).
    MetricSummary(Symbol),
    /// Running summary statistics for a market-specific metric (`MetricSummary`).
    MarketMetricSummary(Symbol, Symbol),
}
