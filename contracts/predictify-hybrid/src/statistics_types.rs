#![allow(dead_code)]

use crate::market_types::MarketState;
use soroban_sdk::{contracttype, Address, Map, String, Symbol, Vec};

// ===== STATISTICS TYPES =====

/// Platform-wide usage statistics
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformStatistics {
    /// Total number of markets/events created
    pub total_events_created: u64,
    /// Total number of bets placed
    pub total_bets_placed: u64,
    /// Total volume (amount wagered) in token units
    pub total_volume: i128,
    /// Total fees collected in token units
    pub total_fees_collected: i128,
    /// Number of currently active (non-resolved) events
    pub active_events_count: u32,
}

/// User-specific betting statistics
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserStatistics {
    /// Total number of bets placed by the user
    pub total_bets_placed: u64,
    /// Total amount wagered by the user
    pub total_amount_wagered: i128,
    /// Total winnings claimed by the user
    pub total_winnings: i128,
    /// Number of winning bets
    pub total_bets_won: u64,
    /// Win rate in basis points (0-10000, 100% = 10000)
    pub win_rate: u32,
    /// Timestamp of last activity
    pub last_activity_ts: u64,
}

// ===== DASHBOARD STATISTICS TYPES =====

/// Market statistics optimized for dashboard display
///
/// Provides aggregated market-level metrics for UI rendering.
/// Versioned to allow field additions without breaking client compatibility.
///
/// # Stability
///
/// This type is versioned (`V1`) and will remain backward-compatible.
/// New fields may be added in future versions without removing existing ones.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketStatisticsV1 {
    /// Market identifier
    pub market_id: Symbol,
    /// Number of unique participants who placed bets
    pub participant_count: u32,
    /// Total amount staked in the market
    pub total_volume: i128,
    /// Average stake per participant
    pub average_stake: i128,
    /// Consensus strength (0-10000, where 10000 = perfect consensus)
    /// Calculated as: (max_outcome_stake / total_volume) * 10000
    pub consensus_strength: u32,
    /// Market volatility (0-10000)
    /// Measures stake distribution across outcomes
    /// Higher = more distributed (less consensus), Lower = concentrated
    pub volatility: u32,
    /// Current market state
    pub state: MarketState,
    /// Timestamp when market was created
    pub created_at: u64,
    /// Market question
    pub question: String,
    /// Query API version (always 1 for this type)
    pub api_version: u32,
}

/// User leaderboard entry for dashboard ranking
///
/// Represents a user's position in rankings by winnings, win rate, etc.
/// Used for leaderboard displays and user achievements.
///
/// # Stability
///
/// Versioned type - backward compatible for future additions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserLeaderboardEntryV1 {
    /// User address
    pub user: Address,
    /// Rank in this leaderboard (1-indexed)
    pub rank: u32,
    /// Total winnings claimed
    pub total_winnings: i128,
    /// Win rate in basis points (0-10000)
    pub win_rate: u32,
    /// Total bets placed by user
    pub total_bets_placed: u64,
    /// Number of winning bets
    pub winning_bets: u64,
    /// Total amount wagered
    pub total_wagered: i128,
    /// Last activity timestamp
    pub last_activity: u64,
}

/// Per-market top-N leaderboard entry, ranked by cumulative stake in that market.
///
/// This type is stored inside the bounded heap at
/// `DataKey::MarketLeaderboard(market_id)`.  The heap never grows beyond
/// `MAX_MARKET_LEADERBOARD_CAPACITY` (50) entries, so storage costs are fixed.
///
/// ## Ranking key
///
/// Entries are ranked **descending by `stake`**.  Two users with equal stake are
/// compared by `last_bet_timestamp` as a secondary key (earlier first gets the
/// higher position, preserving first-bettor advantage on ties).
///
/// # Stability
///
/// Versioned suffix (`V1`) reserved for non-breaking future fields.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketLeaderboardEntry {
    /// Address of the participant.
    pub user: Address,
    /// 1-indexed rank within this market's leaderboard (assigned at read time).
    pub rank: u32,
    /// Total amount staked by this user in this market (cumulative across bets).
    pub stake: i128,
    /// Ledger timestamp of the user's most recent bet in this market.
    pub last_bet_timestamp: u64,
}

/// Category statistics for filtered dashboard views
///
/// Aggregates metrics per market category (e.g., "sports", "crypto", "politics")
/// for category-filtered dashboards.
///
/// # Stability
///
/// Versioned for future extensibility.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryStatisticsV1 {
    /// Category name
    pub category: String,
    /// Number of markets in this category
    pub market_count: u32,
    /// Total volume across all markets in category
    pub total_volume: i128,
    /// Total unique participants in this category
    pub participant_count: u32,
    /// Number of resolved markets
    pub resolved_count: u32,
    /// Average market volume
    pub average_market_volume: i128,
}

/// Versioned dashboard statistics response for platform aggregates
///
/// Provides comprehensive platform-level metrics with version information
/// for client compatibility management.
///
/// # Stability
///
/// This wrapper allows adding new aggregate types in future versions
/// without breaking existing client queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardStatisticsV1 {
    /// API version (always 1 for this type)
    pub api_version: u32,
    /// Platform statistics snapshot
    pub platform_stats: PlatformStatistics,
    /// Ledger timestamp when this query was executed
    pub query_timestamp: u64,
    /// Number of users with activity (if tracked)
    pub active_user_count: u32,
    /// Total value locked in all markets
    pub total_value_locked: i128,
}

/// Comprehensive statistics tracking for market extension usage and effectiveness.
///
/// This structure captures detailed metrics about market extensions, including
/// usage patterns, effectiveness measurements, and impact on market participation.
/// It provides valuable insights for optimizing extension policies and understanding
/// user behavior in prediction markets.
///
/// # Statistics Categories
///
/// **Usage Metrics:**
/// - **Total Extensions**: Number of extensions requested
/// - **Approved Extensions**: Number of extensions approved
/// - **Auto Extensions**: System-initiated extensions
/// - **User Extensions**: Community-requested extensions
///
/// **Effectiveness Metrics:**
/// - **Participation Increase**: Additional votes/stakes after extension
/// - **Resolution Quality**: Impact on market resolution accuracy
/// - **User Satisfaction**: Community feedback on extensions
///
/// **Financial Metrics:**
/// - **Total Fees Collected**: Revenue from extension fees
/// - **Average Fee**: Mean fee per extension request
/// - **Fee Effectiveness**: Correlation between fee and participation
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStats {
    /// Total number of extensions
    pub total_extensions: u32,
    /// Total extension days
    pub total_extension_days: u32,
    /// Maximum extension days allowed
    pub max_extension_days: u32,
    /// Whether the market can be extended
    pub can_extend: bool,
    /// Extension fee per day
    pub extension_fee_per_day: i128,
}
