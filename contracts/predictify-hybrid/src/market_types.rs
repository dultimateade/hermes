#![allow(dead_code)]

use crate::oracle_types::OracleConfig;
use crate::support_types::ClaimInfo;
use crate::Error;
use crate::timelock::MarketTimelockConfig;
use soroban_sdk::{contracttype, xdr::ToXdr, Address, BytesN, Env, Map, String, Vec};

// ===== MARKET STATE =====

/// Enumeration of possible market states throughout the prediction market lifecycle.
///
/// This enum defines the various states a prediction market can be in, from initial
/// creation through final resolution and closure. Each state represents a distinct
/// phase with specific business rules, available operations, and state transition
/// requirements.
///
/// # State Lifecycle
///
/// The typical market progression follows this pattern:
/// ```text
/// Active → Ended → [Disputed] → Resolved → Closed
/// ```
///
/// **Alternative flows:**
/// - **Cancellation**: `Active → Cancelled` (emergency situations)
/// - **Direct Resolution**: `Active → Resolved` (admin override)
/// - **Dispute Flow**: `Ended → Disputed → Resolved`
///
/// # State Descriptions
///
/// **Active**: Market is live and accepting user participation
/// - Users can place votes and stakes
/// - Market question and outcomes are fixed
/// - Oracle configuration is immutable
/// - Voting period is ongoing
///
/// **Ended**: Market voting period has concluded
/// - No new votes or stakes accepted
/// - Oracle resolution can be triggered
/// - Community consensus can be calculated
/// - Dispute period may be active
///
/// **Disputed**: Market resolution is under dispute
/// - Formal dispute process is active
/// - Additional evidence may be collected
/// - Dispute resolution mechanisms engaged
/// - Final outcome pending dispute resolution
///
/// **Resolved**: Market outcome has been determined
/// - Final outcome is established
/// - Payouts can be calculated and distributed
/// - Resolution method and confidence recorded
/// - Market moves toward closure
///
/// **Closed**: Market is permanently closed
/// - All payouts have been distributed
/// - No further operations allowed
/// - Market data preserved for historical analysis
/// - Final state for completed markets
///
/// **Cancelled**: Market has been cancelled
/// - Emergency cancellation due to issues
/// - Stakes returned to participants
/// - No winner determination
/// - Administrative action required
///
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketState {
    /// Market is active and accepting votes
    Active,
    /// Market has ended, waiting for resolution
    Ended,
    /// Market is under dispute
    Disputed,
    /// Market has been resolved
    Resolved,
    /// Market is closed
    Closed,
    /// Market has been cancelled
    Cancelled,
}

/// Comprehensive market data structure representing a complete prediction market.
///
/// This structure contains all data necessary to manage a prediction market throughout
/// its entire lifecycle, from creation through resolution and payout distribution.
/// It serves as the central data model for all market operations and state management.
///
/// # Core Market Components
///
/// **Market Identity:**
/// - **Admin**: Market administrator with special privileges
/// - **Question**: The prediction question being resolved
/// - **Outcomes**: Available outcomes users can vote on
/// - **End Time**: When the voting period concludes
///
/// **Oracle Integration:**
/// - **Oracle Config**: Configuration for oracle-based resolution
/// - **Oracle Result**: Final oracle outcome (set after resolution)
///
/// **User Participation:**
/// - **Votes**: User outcome predictions
/// - **Stakes**: User financial commitments
/// - **Claimed**: Payout claim status tracking
///
/// **Financial Tracking:**
/// - **Total Staked**: Aggregate stake amount across all users
/// - **Dispute Stakes**: Stakes committed to dispute processes
/// - **Market State**: Current lifecycle state
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Market {
    /// Market administrator address
    pub admin: Address,
    /// Market question/prediction
    pub question: String,
    /// Available outcomes for the market
    pub outcomes: Vec<String>,
    /// Market end time (Unix timestamp)
    pub end_time: u64,
    /// Oracle configuration for this market (primary)
    pub oracle_config: OracleConfig,
    /// SHA-256 commitment to the canonical public market metadata.
    ///
    /// The committed fields are `question`, `outcomes`, and the primary
    /// `oracle_config`, encoded with Soroban's canonical XDR serializer before
    /// hashing. Clients can recompute this value off-chain and call
    /// `verify_market_metadata` to detect tampered storage reads or stale caches.
    pub metadata_commitment: BytesN<32>,
    /// Whether a fallback oracle is configured.
    ///
    /// When `true`, automatic resolution attempts the primary oracle once and then this market's
    /// fallback oracle once if the primary attempt fails.
    pub has_fallback: bool,
    /// Fallback oracle configuration.
    ///
    /// When `has_fallback` is `false`, this field is populated with `OracleConfig::none_sentinel()`
    /// so the stored representation stays collision-free without using `Option<OracleConfig>`.
    /// When `has_fallback` is `true`, this config is consulted only after one failed primary attempt.
    pub fallback_oracle_config: OracleConfig,
    /// Resolution timeout in seconds after `end_time`.
    ///
    /// Automatic oracle resolution stops once `ledger.timestamp() >= end_time + resolution_timeout`.
    ///
    /// This duration uses ledger-time seconds. At the default of 86,400 seconds
    /// (24 hours), ledger timestamp drift is negligible. Values below 300 seconds
    /// are inadvisable because drift may represent a meaningful fraction of the
    /// window. See `docs/TIMESTAMP_ASSUMPTIONS.md`.
    pub resolution_timeout: u64,
    /// Oracle result (set after market ends)
    pub oracle_result: Option<String>,
    /// User votes mapping (address -> outcome)
    pub votes: Map<Address, String>,
    /// User stakes mapping (address -> stake amount)
    pub stakes: Map<Address, i128>,
    /// Claimed status mapping (address -> ClaimInfo with timestamp and payout tracking)
    pub claimed: Map<Address, ClaimInfo>,
    /// Total amount staked in the market
    pub total_staked: i128,
    /// Dispute stakes mapping (address -> dispute stake)
    pub dispute_stakes: Map<Address, i128>,
    /// Winning outcome(s) (set after resolution)
    /// For single winner: contains one outcome
    /// For ties/multi-winner: contains multiple outcomes (pool split among winners)
    pub winning_outcomes: Option<Vec<String>>,
    /// Whether fees have been collected
    pub fee_collected: bool,
    /// Current market state
    pub state: MarketState,

    /// Total extension days
    pub total_extension_days: u32,
    /// Maximum extension days allowed
    pub max_extension_days: u32,

    /// Extension history
    pub extension_history: Vec<crate::support_types::MarketExtension>,

    /// Optional category for the event (e.g., "sports", "crypto", "politics").
    ///
    /// When set, the string must respect configured min/max length (see `config` and `metadata_limits`); `None` means
    /// unset. `Some` with an empty string is invalid.
    pub category: Option<String>,
    /// Searchable tags for filtering: bounded list length, per-tag length, and no duplicates (enforced in
    /// `metadata_limits::validate_event_tags` and `Market::validate`).
    pub tags: Vec<String>,
    /// Minimum total pool size required for resolution (None = no minimum)
    pub min_pool_size: Option<i128>,
    /// Bet deadline (Unix timestamp). No bets accepted after this time. 0 = use end_time (no early cutoff).
    pub bet_deadline: u64,
    /// Dispute window in seconds after end_time.
    ///
    /// Payouts are allowed only after `end_time + dispute_window_seconds` has
    /// passed (or the dispute is resolved). This duration uses ledger-time
    /// seconds. At the default of 86,400 seconds, ledger timestamp drift is
    /// negligible. Values below 3,600 seconds (1 hour) are inadvisable.
    /// See `docs/TIMESTAMP_ASSUMPTIONS.md`.
    pub dispute_window_seconds: u64,
    /// Whether unclaimed winnings have already been swept for this market.
    /// Set to true after the first successful sweep to prevent double-crediting the treasury.
    pub winnings_swept: bool,
    /// Per-market timelock configuration for admin actions.
    pub timelock_config: MarketTimelockConfig,
    /// Per-market dispute stake floor (None = use global/default minimum)
    pub dispute_stake_floor: Option<i128>,
    /// Maximum number of unique participants allowed (None = no limit).
    pub max_participants: Option<u32>,

    /// Per-market minimum bet amount in base token units (e.g. stroops).
    ///
    /// When `Some(amount)`, any individual bet on this market whose `amount`
    /// is less than this value is rejected with [`Error::BetBelowMarketMin`].
    /// The check is applied **after** the global/per-event [`BetLimits`]
    /// `min_bet`, so the effective floor is `max(global_min, min_bet_amount)`.
    ///
    /// `None` means no per-market override — the global/per-event minimum applies.
    /// Must be positive when `Some`; zero or negative values are rejected with
    /// [`Error::InvalidInput`] during `set_min_bet`.
    pub min_bet_amount: Option<i128>,
}

/// Canonical payload committed by `Market::metadata_commitment`.
///
/// Keep this type small and purpose-built so commitment review is simple. The
/// field order is part of the commitment definition; changing it requires a
/// documented migration/versioning plan for clients.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketMetadataCommitmentPayload {
    pub question: String,
    pub outcomes: Vec<String>,
    pub oracle_config: OracleConfig,
}

/// Community consensus data structure for tracking collective market resolution.
///
/// This structure captures the community's collective opinion on market outcomes,
/// providing an alternative or supplementary resolution method to oracle-based
/// resolution. It aggregates user votes, stakes, and participation to determine
/// the community's consensus on the correct market outcome.
///
/// # Consensus Components
///
/// **Outcome Data:**
/// - **Outcome**: The consensus outcome determined by the community
/// - **Votes**: Number of individual votes for this outcome
/// - **Total Votes**: Total number of votes across all outcomes
/// - **Percentage**: Percentage of votes for this outcome
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityConsensus {
    /// Consensus outcome
    pub outcome: String,
    /// Number of votes for this outcome
    pub votes: u32,
    /// Total number of votes
    pub total_votes: u32,
    /// Percentage of votes for this outcome
    pub percentage: i128,
}

///////////////////////////////////////////////////
/// Market pause information tracking   //////////
/////////////////////////////////////////////////
#[contracttype]
#[derive(Clone, Debug)]
pub struct MarketPauseInfo {
    pub is_paused: bool,
    pub paused_at: u64,
    pub pause_duration_hours: u32,
    pub paused_by: Address,
    pub pause_end_time: u64,
    pub original_state: MarketState,
    pub reason: PauseReason,
}

///////////////////////////////////////////////////
/// Pause Reason   //////////
/////////////////////////////////////////////////
#[contracttype]
pub enum PauseReason {
    /// Market paused due to emergency or security issue
    Emergency,
    /// Market paused for regulatory compliance
    Regulatory,
    /// Market paused for maintenance or upgrade
    Maintenance,
    /// Market paused due to oracle issues
    OracleIssue,
    /// Market paused due to liquidity concerns
    LiquidityConcern,
    /// Market paused due to suspicious activity
    SuspiciousActivity,
    /// Market paused with custom reason (string)
    Custom(String),
}

/// Market/event status enumeration for queries.
///
/// Simplified status enumeration optimized for query responses,
/// mapping internal market states to user-friendly statuses.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarketStatus {
    /// Market is open for voting
    Active,
    /// Market voting has ended
    Ended,
    /// Market outcome is disputed
    Disputed,
    /// Market outcome has been resolved
    Resolved,
    /// Market is closed
    Closed,
    /// Market has been cancelled
    Cancelled,
}

impl MarketStatus {
    /// Convert from internal MarketState to query MarketStatus
    pub fn from_market_state(state: MarketState) -> Self {
        match state {
            MarketState::Active => MarketStatus::Active,
            MarketState::Ended => MarketStatus::Ended,
            MarketState::Disputed => MarketStatus::Disputed,
            MarketState::Resolved => MarketStatus::Resolved,
            MarketState::Closed => MarketStatus::Closed,
            MarketState::Cancelled => MarketStatus::Cancelled,
        }
    }
}

impl Market {
    /// Computes the SHA-256 commitment for canonical market metadata.
    ///
    /// The commitment covers only public creation metadata that off-chain UIs
    /// display as market identity: `question`, `outcomes`, and primary
    /// `oracle_config`. The payload is serialized with Soroban's canonical XDR
    /// representation before hashing, avoiding ambiguous delimiters or host-side
    /// formatting differences.
    pub fn compute_metadata_commitment(
        env: &Env,
        question: &String,
        outcomes: &Vec<String>,
        oracle_config: &OracleConfig,
    ) -> BytesN<32> {
        let payload = MarketMetadataCommitmentPayload {
            question: question.clone(),
            outcomes: outcomes.clone(),
            oracle_config: oracle_config.clone(),
        };
        env.crypto().sha256(&payload.to_xdr(env)).into()
    }

    /// Recomputes and stores this market's metadata commitment after an
    /// authorized metadata update.
    pub fn refresh_metadata_commitment(&mut self, env: &Env) {
        self.metadata_commitment = Self::compute_metadata_commitment(
            env,
            &self.question,
            &self.outcomes,
            &self.oracle_config,
        );
    }

    /// Verifies that `expected` matches both the stored commitment and a fresh
    /// commitment over the currently stored fields.
    ///
    /// Comparing against a fresh commitment means the helper returns `false` if
    /// any committed field was mutated without also updating the commitment.
    pub fn verify_metadata_commitment(&self, env: &Env, expected: &BytesN<32>) -> bool {
        let actual = Self::compute_metadata_commitment(
            env,
            &self.question,
            &self.outcomes,
            &self.oracle_config,
        );
        actual.eq(expected) && self.metadata_commitment.eq(expected)
    }

    /// Create a new market
    pub fn new(
        env: &Env,
        admin: Address,
        question: String,
        outcomes: Vec<String>,
        end_time: u64,
        oracle_config: OracleConfig,
        fallback_oracle_config: Option<OracleConfig>,
        resolution_timeout: u64,
        state: MarketState,
    ) -> Self {
        let (has_fallback, fallback_cfg) = match &fallback_oracle_config {
            Some(c) => (true, c.clone()),
            None => (false, OracleConfig::none_sentinel(env)),
        };
        let metadata_commitment = Self::compute_metadata_commitment(
            env,
            &question,
            &outcomes,
            &oracle_config,
        );
        Self {
            admin,
            question,
            outcomes,
            end_time,
            oracle_config,
            metadata_commitment,
            has_fallback,
            fallback_oracle_config: fallback_cfg,
            resolution_timeout,
            oracle_result: None,
            votes: Map::new(env),
            stakes: Map::new(env),
            claimed: Map::new(env),
            total_staked: 0,
            dispute_stakes: Map::new(env),
            winning_outcomes: None,
            fee_collected: false,
            state,

            total_extension_days: 0,
            max_extension_days: 30, // Default maximum extension days
            extension_history: Vec::new(env),

            category: None,
            tags: Vec::new(env),
            min_pool_size: None,
            bet_deadline: 0,
            dispute_window_seconds: 86400, // 24h default
            winnings_swept: false,
            timelock_config: MarketTimelockConfig::default(),
            max_participants: None,
            dispute_stake_floor: None,
            max_participants: None,
        }
    }

    /// Check if the market is active (not ended) using the current ledger timestamp
    pub fn is_active(&self, env: &Env) -> bool {
        env.ledger().timestamp() < self.end_time
    }

    /// Check if the market has ended using the current ledger timestamp
    pub fn has_ended(&self, env: &Env) -> bool {
        env.ledger().timestamp() >= self.end_time
    }

    /// Check if the market is resolved
    pub fn is_resolved(&self) -> bool {
        self.winning_outcomes.is_some()
    }

    /// Get the primary winning outcome (first outcome if multiple, for backward compatibility)
    pub fn get_winning_outcome(&self) -> Option<String> {
        self.winning_outcomes.as_ref().and_then(|outcomes| {
            if outcomes.len() > 0 {
                Some(outcomes.get(0).unwrap().clone())
            } else {
                None
            }
        })
    }

    /// Check if a specific outcome is a winner (handles both single and multi-winner cases)
    pub fn is_winning_outcome(&self, outcome: &String) -> bool {
        self.winning_outcomes
            .as_ref()
            .map(|outcomes| outcomes.contains(outcome))
            .unwrap_or(false)
    }

    /// Get total dispute stakes for the market
    pub fn total_dispute_stakes(&self) -> i128 {
        let mut total = 0;
        for (_, stake) in self.dispute_stakes.iter() {
            total += stake;
        }
        total
    }

    /// Add a vote to the market (for testing)
    pub fn add_vote(&mut self, user: Address, outcome: String, stake: i128) {
        self.votes.set(user.clone(), outcome);
        self.stakes.set(user, stake);
        self.total_staked += stake;
    }

    /// Validate market parameters
    pub fn validate(&self, env: &Env) -> Result<(), crate::Error> {
        // Validate question
        if self.question.is_empty() {
            return Err(crate::Error::InvalidQuestion);
        }

        // Validate question length
        crate::metadata_limits::validate_question_length(&self.question)?;

        // Validate outcomes
        if self.outcomes.len() < 2 {
            return Err(crate::Error::InvalidOutcomes);
        }

        // Validate outcomes count
        crate::metadata_limits::validate_outcomes_count(&self.outcomes)?;

        // Validate each outcome length
        crate::metadata_limits::validate_outcomes_length(&self.outcomes)?;

        // Validate primary oracle config
        // Validate primary oracle config
        self.oracle_config.validate(env)?;

        // FIX: only validate fallback if it's actually provided (not sentinel)
        if self.has_fallback && !self.fallback_oracle_config.is_none_sentinel() {
            if self.fallback_oracle_config.feed_id.is_empty() {
                return Err(crate::Error::InvalidOracleFeed);
            }

            self.fallback_oracle_config.validate(env)?;
        }

        // Validate end time
        if self.end_time <= env.ledger().timestamp() {
            return Err(crate::Error::InvalidDuration);
        }
        // Optional category and tags: size limits and duplicate-tag rejection (see metadata_limits)
        crate::metadata_limits::validate_option_category_metadata(&self.category)?;
        crate::metadata_limits::validate_event_tags(&self.tags)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn valid_oracle_config(env: &Env) -> OracleConfig {
        OracleConfig::new(
            crate::oracle_types::OracleProvider::Reflector,
            Address::generate(env),
            String::from_str(env, "BTC/USD"),
            1_000_000,
            String::from_str(env, "gt"),
        )
    }

    #[test]
    fn market_validate_rejects_invalid_fallback_when_enabled() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let outcomes = Vec::from_array(
            &env,
            [String::from_str(&env, "yes"), String::from_str(&env, "no")],
        );

        let invalid_fallback = OracleConfig::new(
            crate::oracle_types::OracleProvider::Pyth,
            Address::generate(&env),
            String::from_str(&env, "BTC/USD"),
            1_000_000,
            String::from_str(&env, "gt"),
        );

        let market = Market::new(
            &env,
            admin,
            String::from_str(&env, "Will BTC be above $10,000?"),
            outcomes,
            env.ledger().timestamp() + 1000,
            valid_oracle_config(&env),
            Some(invalid_fallback),
            3600,
            MarketState::Active,
        );

        assert_eq!(market.validate(&env), Err(crate::Error::InvalidOracleFeed));
    }

    #[test]
    fn market_validate_allows_no_fallback_sentinel_path() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let outcomes = Vec::from_array(
            &env,
            [String::from_str(&env, "yes"), String::from_str(&env, "no")],
        );

        let market = Market::new(
            &env,
            admin,
            String::from_str(&env, "Will BTC be above $10,000?"),
            outcomes,
            env.ledger().timestamp() + 1000,
            valid_oracle_config(&env),
            None,
            3600,
            MarketState::Active,
        );

        assert_eq!(market.validate(&env), Ok(()));
    }
}
