#![allow(dead_code)]

use soroban_sdk::{contracttype, Address, Env, Map, String, Symbol};

// ===== BET PLACEMENT TYPES =====

/// Status of a bet placed on a prediction market.
///
/// This enum tracks the lifecycle of a bet from placement through resolution:
/// - `Active`: Bet is placed and funds are locked, awaiting market resolution
/// - `Won`: Market resolved in favor of user's predicted outcome, winnings claimable
/// - `Lost`: Market resolved against user's predicted outcome, funds forfeited
/// - `Refunded`: Bet was refunded due to market cancellation or special circumstances
/// - `Cancelled`: Bet was cancelled before market resolution (if allowed)
///
/// # State Transitions
///
/// ```text
/// Active → Won (market resolved in user's favor)
/// Active → Lost (market resolved against user)
/// Active → Refunded (market cancelled)
/// Active → Cancelled (bet cancelled before resolution, if permitted)
/// ```
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BetStatus {
    /// Bet is active with funds locked
    Active,
    /// Bet won - user predicted correctly
    Won,
    /// Bet lost - user predicted incorrectly
    Lost,
    /// Bet was refunded (market cancelled)
    Refunded,
    /// Bet was cancelled by user (if allowed)
    Cancelled,
}

/// Represents a user's bet on a prediction market event.
///
/// This structure encapsulates all information about a user's bet placement,
/// including the selected outcome, locked funds amount, and bet status.
/// Bets are distinct from votes in that they represent a financial wager
/// on the predicted outcome rather than a governance/consensus vote.
///
/// # Bet vs Vote Distinction
///
/// - **Bet**: Financial wager on predicted outcome with locked funds
/// - **Vote**: Participation in community consensus for market resolution
///
/// # Fund Locking
///
/// When a bet is placed:
/// 1. User's funds (XLM or Stellar tokens) are transferred to the contract
/// 2. Funds remain locked until market resolution
/// 3. Upon resolution:
///    - Winners receive proportional share of total bet pool (minus fees)
///    - Losers forfeit their locked funds
///    - Refunds issued if market is cancelled
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bet {
    /// Address of the user who placed the bet
    pub user: Address,
    /// Market ID this bet is placed on
    pub market_id: Symbol,
    /// Selected outcome the user is betting on
    pub outcome: String,
    /// Amount of funds locked for this bet (in stroops)
    pub amount: i128,
    /// Timestamp when the bet was placed
    pub timestamp: u64,
    /// Current status of the bet
    pub status: BetStatus,
}

impl Bet {
    /// Create a new active bet
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment
    /// * `user` - Address of the user placing the bet
    /// * `market_id` - Symbol identifying the market
    /// * `outcome` - The outcome the user is betting on
    /// * `amount` - The amount to lock for this bet
    ///
    /// # Returns
    ///
    /// A new `Bet` instance with `Active` status and current timestamp
    pub fn new(env: &Env, user: Address, market_id: Symbol, outcome: String, amount: i128) -> Self {
        Self {
            user,
            market_id,
            outcome,
            amount,
            timestamp: env.ledger().timestamp(),
            status: BetStatus::Active,
        }
    }

    /// Check if the bet is still active (funds locked, awaiting resolution)
    pub fn is_active(&self) -> bool {
        self.status == BetStatus::Active
    }

    /// Check if the bet has been resolved (won or lost)
    pub fn is_resolved(&self) -> bool {
        matches!(self.status, BetStatus::Won | BetStatus::Lost)
    }

    /// Check if the user won this bet
    pub fn is_winner(&self) -> bool {
        self.status == BetStatus::Won
    }

    /// Mark the bet as won
    pub fn mark_as_won(&mut self) {
        self.status = BetStatus::Won;
    }

    /// Mark the bet as lost
    pub fn mark_as_lost(&mut self) {
        self.status = BetStatus::Lost;
    }

    /// Mark the bet as refunded
    pub fn mark_as_refunded(&mut self) {
        self.status = BetStatus::Refunded;
    }

    /// Mark the bet as cancelled
    pub fn mark_as_cancelled(&mut self) {
        self.status = BetStatus::Cancelled;
    }
}

/// Configurable minimum and maximum bet amount for an event or globally.
///
/// Used to bound bets so markets remain fair and liquid. Admin can set
/// global defaults or per-event limits at creation or via set_bet_limits.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BetLimits {
    /// Minimum bet amount (in base token units, e.g. stroops)
    pub min_bet: i128,
    /// Maximum bet amount (in base token units)
    pub max_bet: i128,
}

/// Statistics for bets placed on a specific market.
///
/// This structure provides aggregate information about betting activity
/// on a market, useful for analytics, UI display, and market health assessment.
///
/// # Example Usage
///
/// ```rust
/// # use soroban_sdk::{Env, Map, String};
/// # use predictify_hybrid::types::BetStats;
/// # let env = Env::default();
///
/// let mut outcome_totals = Map::new(&env);
/// outcome_totals.set(String::from_str(&env, "yes"), 50_000_000i128);
/// outcome_totals.set(String::from_str(&env, "no"), 30_000_000i128);
///
/// let stats = BetStats {
///     total_bets: 15,
///     total_amount_locked: 80_000_000, // 8 XLM
///     unique_bettors: 12,
///     outcome_totals,
/// };
///
/// println!("Total bets: {}", stats.total_bets);
/// println!("Total locked: {} stroops", stats.total_amount_locked);
/// println!("Unique bettors: {}", stats.unique_bettors);
/// ```
#[contracttype]
#[derive(Clone, Debug)]
pub struct BetStats {
    /// Total number of bets placed on this market
    pub total_bets: u32,
    /// Total amount of funds locked across all bets
    pub total_amount_locked: i128,
    /// Number of unique users who placed bets
    pub unique_bettors: u32,
    /// Total amount locked per outcome
    pub outcome_totals: Map<String, i128>,
}
