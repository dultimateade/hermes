//! # Betting crate
//!
//! The `betting` crate is a thin, indexing-focused wrapper around the
//! betting subsystems of [`predictify_hybrid`].  Its purpose is to expose a
//! **stable, structured, off-chain-indexer-friendly event surface** for
//! the bet lifecycle:
//!
//! * creation (single and batch),
//! * status transitions (resolve / cancel / refund),
//! * payout (claim),
//! * aggregated statistics updates.
//!
//! Every event in this module is:
//! * versioned through [`events::BettingEventSchema`]: the
//!   deployment-wide `schema_version` is encoded **once** in the contract
//!   instance record (see [`events::BettingEventSchema::ensure_schema_record`])
//!   and read back with
//!   [`events::BettingEventSchema::deployment_schema_version`], so it is not
//!   repeated in the topic tuple of every event,
//! * monotonically nonce-stamped per topic so out-of-order / replayed
//!   events from any indexer source can be detected,
//! * stamped with the Soroban ledger timestamp so wall-clock-independent
//!   sequences can be reconstructed.
//!
//! See [`events`] for the public API.

#![no_std]

extern crate alloc;

pub mod events;

/// Default instance storage TTL bump in ledgers (~30 days).
pub const INSTANCE_TTL_LEDGERS: u32 = 535_680;

/// Minimum oracle confidence score (in basis points, `0..=10_000`) required
/// before a market may be finalised during resolution.
///
/// High-volatility assets can produce oracle readings that are technically
/// valid but carry low confidence.  A non-zero minimum confidence prevents
/// finalising markets on such noisy data.  The default of `0` preserves the
/// historical behaviour of accepting any valid reading.
pub const DEFAULT_MIN_CONFIDENCE_BPS: u32 = 0;

/// Upper bound for a confidence score expressed in basis points.
pub const MAX_CONFIDENCE_BPS: u32 = 10_000;

/// Error returned when a resolution is attempted with an oracle reading whose
/// confidence is below the configured minimum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionError {
    /// The oracle reading's confidence score is below the configured minimum.
    ConfidenceBelowMinimum {
        /// The confidence reported by the oracle, in basis points.
        confidence_bps: u32,
        /// The configured minimum confidence, in basis points.
        min_confidence_bps: u32,
    },
}

/// Configuration governing how strict market resolution is with respect to
/// oracle confidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionConfig {
    /// Minimum confidence (in basis points) an oracle reading must report for
    /// the market to be finalised.  `0` disables the check.
    pub min_confidence_bps: u32,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            min_confidence_bps: DEFAULT_MIN_CONFIDENCE_BPS,
        }
    }
}

impl ResolutionConfig {
    /// Build a config with an explicit minimum confidence threshold.
    ///
    /// Values above [`MAX_CONFIDENCE_BPS`] are clamped so a misconfigured
    /// caller cannot accidentally make resolution impossible.
    pub fn with_min_confidence(min_confidence_bps: u32) -> Self {
        Self {
            min_confidence_bps: min_confidence_bps.min(MAX_CONFIDENCE_BPS),
        }
    }

    /// Returns `true` when a reading with `confidence_bps` is acceptable for
    /// finalisation under this configuration.
    ///
    /// A minimum of `0` accepts every reading, preserving backward
    /// compatibility with deployments that do not configure a threshold.
    pub fn accepts(&self, confidence_bps: u32) -> bool {
        self.min_confidence_bps == 0 || confidence_bps >= self.min_confidence_bps
    }

    /// Validate an oracle reading's confidence against this configuration,
    /// returning [`ResolutionError::ConfidenceBelowMinimum`] when the reading
    /// is too noisy to finalise a market.
    pub fn check_confidence(&self, confidence_bps: u32) -> Result<(), ResolutionError> {
        if self.accepts(confidence_bps) {
            Ok(())
        } else {
            Err(ResolutionError::ConfidenceBelowMinimum {
                confidence_bps,
                min_confidence_bps: self.min_confidence_bps,
            })
        }
    }
}
