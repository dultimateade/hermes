#![allow(dead_code)]

use crate::market_types::MarketState;
use soroban_sdk::{contracttype, Address, Env, String, Vec};

// ===== EVENT TYPES =====

/// Visibility setting for events (public vs private)
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventVisibility {
    /// Public event - anyone can bet
    Public,
    /// Private event - only allowlisted addresses can bet
    Private,
}

/// Represents a prediction market event with specified parameters.
///
/// This structure stores all metadata and configuration for a prediction event,
/// including its description, possible outcomes, timing, and oracle integration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    /// Unique identifier for the event
    pub id: soroban_sdk::Symbol,
    /// Event description or question
    pub description: String,
    /// Possible outcomes for the event (e.g., ["yes", "no"])
    pub outcomes: Vec<String>,
    /// When the event ends (Unix timestamp)
    pub end_time: u64,
    /// Oracle configuration for result verification (primary)
    pub oracle_config: crate::oracle_types::OracleConfig,
    /// Whether a fallback oracle is configured.
    ///
    /// When `true`, automatic resolution attempts the primary oracle once and then this event's
    /// fallback oracle once if the primary attempt fails.
    pub has_fallback: bool,
    /// Fallback oracle configuration.
    ///
    /// When `has_fallback` is `false`, this field is populated with `OracleConfig::none_sentinel()`
    /// so the stored representation stays collision-free without using `Option<OracleConfig>`.
    /// When `has_fallback` is `true`, this config is consulted only after one failed primary attempt.
    pub fallback_oracle_config: crate::oracle_types::OracleConfig,
    /// Resolution timeout in seconds after `end_time`.
    ///
    /// Automatic oracle resolution stops once `ledger.timestamp() >= end_time + resolution_timeout`.
    ///
    /// This duration uses ledger-time seconds. At the default of 86,400 seconds
    /// (24 hours), ledger timestamp drift is negligible. Values below 300 seconds
    /// are inadvisable because drift may represent a meaningful fraction of the
    /// window. See `docs/TIMESTAMP_ASSUMPTIONS.md`.
    pub resolution_timeout: u64,
    /// Administrative address that created/manages the event
    pub admin: Address,
    /// When the event was created (Unix timestamp)
    pub created_at: u64,
    /// Current status of the event
    pub status: MarketState,
    /// Visibility setting (public or private)
    pub visibility: EventVisibility,
    /// Allowlist of addresses permitted to bet (only enforced for private events)
    pub allowlist: Vec<Address>,
}

/// Summary of an event (market) for historical queries and analytics.
///
/// Contains only public metadata and outcome; no sensitive data (no votes, stakes, or addresses).
/// Used by `query_events_history`, `query_events_by_resolution_status`, and `query_events_by_category`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHistoryEntry {
    /// Market/event ID
    pub market_id: soroban_sdk::Symbol,
    /// Question text (public)
    pub question: String,
    /// Outcome names (public)
    pub outcomes: Vec<String>,
    /// Market end time (Unix timestamp)
    pub end_time: u64,
    /// Creation timestamp (from registry)
    pub created_at: u64,
    /// Current market state (Active, Resolved, Cancelled, etc.)
    pub state: MarketState,
    /// Winning outcome if resolved
    pub winning_outcome: Option<String>,
    /// Total amount staked (public aggregate)
    pub total_staked: i128,
    /// When archived (if any); None if not archived
    pub archived_at: Option<u64>,
    /// Category / feed identifier (e.g. oracle feed_id) for filtering
    pub category: String,
    /// List of tags for filtering events by multiple dimensions
    pub tags: Vec<String>,
}
