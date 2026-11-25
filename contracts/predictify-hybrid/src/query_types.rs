#![allow(dead_code)]

use crate::market_types::MarketStatus;
use soroban_sdk::{contracttype, Address, Map, String, Symbol, Vec};

// ===== QUERY RESPONSE TYPES =====

/// Comprehensive event/market details query response.
///
/// This structure contains complete information about a prediction market,
/// suitable for client-side display and analysis. All fields are structured
/// for easy serialization and client consumption.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventDetailsQuery {
    /// Market/event ID
    pub market_id: Symbol,
    /// Prediction question
    pub question: String,
    /// Possible outcomes
    pub outcomes: Vec<String>,
    /// Market creation timestamp
    pub created_at: u64,
    /// Market end timestamp
    pub end_time: u64,
    /// Current market status
    pub status: MarketStatus,
    /// Oracle provider used for resolution
    pub oracle_provider: String,
    /// Price feed identifier
    pub feed_id: String,
    /// Total amount staked in market
    pub total_staked: i128,
    /// Current winning outcome (if resolved)
    pub winning_outcome: Option<String>,
    /// Oracle result (if available)
    pub oracle_result: Option<String>,
    /// Number of unique participants
    pub participant_count: u32,
    /// Total number of votes
    pub vote_count: u32,
    /// Market administrator
    pub admin: Address,
}

/// User bet details query response.
///
/// Contains comprehensive information about a user's participation
/// in a specific market, including votes, stakes, and payout eligibility.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserBetQuery {
    /// User address
    pub user: Address,
    /// Market/event ID
    pub market_id: Symbol,
    /// User's chosen outcome
    pub outcome: String,
    /// Amount staked by user (in stroops/XLM cents)
    pub stake_amount: i128,
    /// Timestamp of user's vote
    pub voted_at: u64,
    /// Whether user voted on winning outcome
    pub is_winning: bool,
    /// Whether user has already claimed payout
    pub has_claimed: bool,
    /// Potential payout amount (if winning and not claimed)
    pub potential_payout: i128,
    /// User's dispute stake (if any)
    pub dispute_stake: i128,
}

/// Paginated response containing user bet query rows.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserBetPagedResult {
    pub items: Vec<UserBetQuery>,
    pub next_cursor: u32,
    pub total_count: u32,
}

/// User balance and account status query response.
///
/// Provides comprehensive view of a user's account with current balance
/// and participation metrics across all markets.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserBalanceQuery {
    /// User address
    pub user: Address,
    /// Available balance (in stroops)
    pub available_balance: i128,
    /// Total amount currently staked in active markets
    pub total_staked: i128,
    /// Total amount won from resolved markets
    pub total_winnings: i128,
    /// Number of active bets
    pub active_bet_count: u32,
    /// Number of resolved markets where user participated
    pub resolved_market_count: u32,
    /// Total amount in unclaimed payouts
    pub unclaimed_balance: i128,
}

/// Market pool and liquidity query response.
///
/// Provides detailed information about total stakes and outcome distribution
/// across a market, useful for probability analysis and liquidity assessment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketPoolQuery {
    /// Market/event ID
    pub market_id: Symbol,
    /// Total amount staked across all outcomes
    pub total_pool: i128,
    /// Stake amount for each outcome
    pub outcome_pools: Map<String, i128>,
    /// Platform fees collected
    pub platform_fees: i128,
    /// Implied probability for "yes" outcome (0-100)
    pub implied_probability_yes: u32,
    /// Implied probability for "no" outcome (0-100)
    pub implied_probability_no: u32,
}

/// Contract global state statistics query response.
///
/// Provides system-level metrics and statistics across all markets,
/// useful for dashboard displays and platform monitoring.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStateQuery {
    /// Total number of markets created
    pub total_markets: u32,
    /// Number of currently active markets
    pub active_markets: u32,
    /// Number of resolved markets
    pub resolved_markets: u32,
    /// Total value locked across all markets (in stroops)
    pub total_value_locked: i128,
    /// Total platform fees collected (in stroops)
    pub total_fees_collected: i128,
    /// Number of unique users
    pub unique_users: u32,
    /// Contract version
    pub contract_version: String,
    /// Last contract update timestamp
    pub last_update: u64,
}

/// Multi-market query result for batch operations.
///
/// Container for results when querying multiple markets at once,
/// enabling efficient batch queries with error handling.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultipleBetsQuery {
    /// List of bet queries
    pub bets: Vec<UserBetQuery>,
    /// Total stake across all bets
    pub total_stake: i128,
    /// Total potential payout
    pub total_potential_payout: i128,
    /// Number of winning bets
    pub winning_bets: u32,
}

/// Paginated market ID query result.
///
/// Use `next_cursor` in the next call to continue iteration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedMarketIds {
    /// Market IDs in this page.
    pub items: Vec<Symbol>,
    /// Cursor to pass on the next call (index of the first un-returned item).
    pub next_cursor: u32,
    /// Total number of items available (best-effort; may be approximate for
    /// filtered queries).
    pub total_count: u32,
}

/// Paginated user bet query result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedUserBets {
    /// Bets in this page.
    pub items: Vec<UserBetQuery>,
    /// Cursor to pass on the next call (index of the first un-returned item).
    pub next_cursor: u32,
    /// Total number of items available (best-effort; may be approximate for
    /// filtered queries).
    pub total_count: u32,
}
