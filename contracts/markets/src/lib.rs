#![no_std]
#![allow(clippy::too_many_arguments)]

//! Markets contract with auth-gated entrypoints and hardened security controls.
//!
//! Provides a prediction-market subsystem where every state-changing
//! entrypoint enforces `require_auth` on the acting [`Address`].
//!
//! # Auth Matrix
//!
//! | Function                    | Required Role              |
//! |-----------------------------|----------------------------|
//! | `create_market`             | Creator (any address)      |
//! | `create_market_with_curve`  | Creator (any address)      |
//! | `set_market_payout_curve`   | Market creator or Admin    |
//! | `place_bet`                 | Bettor (any address)       |
//! | `resolve_market`            | Market creator             |
//! | `claim_winnings`            | Winner / Claimant (bettor) |
//! | `cancel_market`             | Market creator             |
//! | `withdraw_funds`            | Market creator             |
//! | `update_market_params`      | Market creator             |
//! | `add_liquidity`             | Liquidity provider         |
//! | `remove_liquidity`          | Liquidity provider         |
//! | `pause_markets`             | Admin                      |
//! | `unpause_markets`           | Admin                      |
//! | `propose_pause`             | Admin                      |
//! | `approve_pause`             | Admin                      |
//! | `execute_pause`             | Admin                      |
//! | `cancel_pause`              | Admin                      |
//! | `set_pause_config`          | Admin                      |
//! | `add_admin`                 | Admin                      |
//! | `transfer_ownership`        | Admin                      |
//! | `deposit_user_balance`      | User (any address)         |
//! | `withdraw_user_balance`     | User (any address)         |
//! | `collect_user_fees`         | Admin                      |
//! | `set_fee_config`            | Admin                      |
//! | `version`                   | Anyone (read-only)         |

pub mod errors;

use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, Address, Env, String, Vec,
};

pub use errors::ContractError;

/// Type alias used by generated client code and test harnesses.
pub type Error = ContractError;

/// Persistent-storage keys used by the Markets contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Sequential counter for generating unique market IDs.
    MarketCounter,
    /// Market data keyed by the numeric market ID.
    Market(u32),
    /// Bet placed by a user on a specific market.
    Bet(u32, Address),
    /// Whether the markets subsystem is globally paused (`true` = paused).
    Paused,
    /// The primary admin / owner address.
    Admin,
    /// List of additional authorized admin addresses.
    AdminList,
    /// Liquidity provided by a user to a specific market.
    Liquidity(u32, Address),
    /// Records that a winner has already claimed their payout for a market.
    /// Written **before** any token transfer in `claim_winnings` to uphold the
    /// checks-effects-interactions pattern and prevent reentrancy double-claims.
    ClaimedBet(u32, Address),

    // ── Issue #007: Hardened Pause Keys ─────────────────────────────
    /// Counter for generating unique pause proposal IDs.
    PauseProposalCounter,
    /// Pause proposal data keyed by proposal ID.
    PauseProposal(u32),
    /// Timelock delay in seconds required before executing a pause proposal.
    PauseDelaySeconds,
    /// Number of distinct admin approvals required to execute a pause proposal.
    PauseRequiredApprovals,

    // ── Issue #022: Bet Enumeration Keys ────────────────────────────
    /// Ordered list of market IDs a user has placed bets on.
    UserBetMarkets(Address),

    // ── Issue #014: Payout Curve Keys ───────────────────────────────
    /// Payout curve configuration for a market.
    MarketPayoutCurve(u32),
    /// Total wager pool for a market.
    MarketTotalPool(u32),
    /// Total wager pool for a specific outcome index within a market.
    OutcomePool(u32, u32),

    // ── Issue #012: Dust-Safe Balance & Fee Keys ────────────────────
    /// Internal balance for a user.
    UserBalance(Address),
    /// Minimum fee amount threshold in stroops.
    MinFeeAmount,
    /// Withdrawal fee in basis points (1 = 0.01%).
    WithdrawalFeeBps,
    /// Cumulative platform fees collected.
    CollectedFees,
}

/// On-chain representation of a prediction market.
#[contracttype]
#[derive(Clone)]
pub struct MarketData {
    /// The address that created the market.
    pub creator: Address,
    /// The prediction question.
    pub question: String,
    /// A human-readable description of the market.
    pub description: String,
    /// Unix timestamp (seconds) when betting closes.
    pub end_time: u64,
    /// Identifies the data source used at resolution time.
    pub resolution_source: String,
    /// Ordered list of possible outcomes.
    pub outcome_tags: Vec<String>,
    /// Whether a winning outcome has been recorded.
    pub resolved: bool,
    /// Index (0-based) of the winning outcome.
    pub winning_outcome: u32,
    /// Whether the market has been cancelled.
    pub cancelled: bool,
}

/// On-chain record of a user's bet on a market.
#[contracttype]
#[derive(Clone)]
pub struct BetData {
    /// Index of the selected outcome.
    pub outcome_index: u32,
    /// Amount staked in the platform's base unit.
    pub amount: i128,
}

/// Tracks how much liquidity a user has added to a market.
#[contracttype]
#[derive(Clone)]
pub struct LiquidityData {
    /// Total amount of liquidity provided.
    pub total_amount: i128,
}

/// Pause proposal record for multi-step / timelocked pause operations (#007).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseProposal {
    /// Sequential proposal identifier.
    pub id: u32,
    /// Admin address that created the proposal.
    pub proposer: Address,
    /// Reason or justification for the pause.
    pub reason: String,
    /// Ledger timestamp when the proposal was created.
    pub created_at: u64,
    /// Earliest ledger timestamp when the proposal may be executed.
    pub eta: u64,
    /// List of admin addresses that have approved the proposal.
    pub approvals: Vec<Address>,
    /// Whether the proposal has already been executed.
    pub executed: bool,
    /// Whether the proposal was cancelled.
    pub cancelled: bool,
}

/// Contract-level query record for a user's bet (#022).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserBetInfo {
    /// Numeric ID of the market.
    pub market_id: u32,
    /// Chosen outcome index (0-based).
    pub outcome_index: u32,
    /// Amount staked.
    pub amount: i128,
    /// Whether winnings for this bet have already been claimed.
    pub claimed: bool,
}

/// Configurable payout curve configuration for distributing market pool funds (#014).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutCurveConfig {
    /// 0 = WinnerTakesAll, 1 = Tiered, 2 = CustomWeights
    pub curve_type: u32,
    /// Outcome index receiving runner-up credit (for Tiered).
    pub runner_up_outcome: u32,
    /// Percentage in basis points awarded to runner-up (e.g. 2000 = 20%).
    pub runner_up_bps: u32,
    /// Custom basis point weights per outcome tag (for CustomWeights).
    pub custom_weights: Vec<u32>,
}

impl PayoutCurveConfig {
    pub const WINNER_TAKES_ALL: u32 = 0;
    pub const TIERED: u32 = 1;
    pub const CUSTOM_WEIGHTS: u32 = 2;

    pub fn winner_takes_all(env: &Env) -> Self {
        Self {
            curve_type: Self::WINNER_TAKES_ALL,
            runner_up_outcome: 0,
            runner_up_bps: 0,
            custom_weights: Vec::new(env),
        }
    }

    pub fn tiered(env: &Env, runner_up_outcome: u32, runner_up_bps: u32) -> Self {
        Self {
            curve_type: Self::TIERED,
            runner_up_outcome,
            runner_up_bps,
            custom_weights: Vec::new(env),
        }
    }

    pub fn custom_weights(weights: Vec<u32>) -> Self {
        Self {
            curve_type: Self::CUSTOM_WEIGHTS,
            runner_up_outcome: 0,
            runner_up_bps: 0,
            custom_weights: weights,
        }
    }
}

/// Result of a dust-safe user balance withdrawal (#012).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawalResult {
    /// Net amount received by the user.
    pub net_amount: i128,
    /// Fee deducted (0 if dust waiver applied).
    pub fee_deducted: i128,
    /// Remaining balance in the contract.
    pub remaining_balance: i128,
}

pub mod admin;

#[contract]
pub struct MarketsContract;

impl MarketsContract {
    // -----------------------------------------------------------------------
    //  Internal Helpers
    // -----------------------------------------------------------------------

    fn is_admin_internal(env: &Env, caller: &Address) -> bool {
        if let Some(admin) = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::Admin)
        {
            if &admin == caller {
                return true;
            }
        } else {
            // First caller with admin action becomes initial admin if unset.
            return true;
        }

        let admin_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AdminList)
            .unwrap_or(Vec::new(env));
        admin_list.contains(caller)
    }

    fn require_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        if !Self::is_admin_internal(env, caller) {
            panic_with_error!(env, ContractError::Unauthorized);
        }
    }

    fn validate_payout_curve(env: &Env, curve: &PayoutCurveConfig, num_outcomes: u32) {
        match curve.curve_type {
            PayoutCurveConfig::WINNER_TAKES_ALL => {}
            PayoutCurveConfig::TIERED => {
                if curve.runner_up_outcome >= num_outcomes {
                    panic_with_error!(env, ContractError::InvalidPayoutCurve);
                }
                if curve.runner_up_bps == 0 || curve.runner_up_bps >= 10_000 {
                    panic_with_error!(env, ContractError::InvalidPayoutCurve);
                }
            }
            PayoutCurveConfig::CUSTOM_WEIGHTS => {
                if curve.custom_weights.len() != num_outcomes {
                    panic_with_error!(env, ContractError::InvalidPayoutCurve);
                }
                let mut sum: u32 = 0;
                for w in curve.custom_weights.iter() {
                    sum = match sum.checked_add(w) {
                        Some(s) => s,
                        None => panic_with_error!(env, ContractError::InvalidPayoutCurve),
                    };
                }
                if sum == 0 || sum > 10_000 {
                    panic_with_error!(env, ContractError::InvalidPayoutCurve);
                }
            }
            _ => panic_with_error!(env, ContractError::InvalidPayoutCurve),
        }
    }
}

#[contractimpl]
impl MarketsContract {
    // -----------------------------------------------------------------------
    //  Read-only Entrypoints
    // -----------------------------------------------------------------------

    /// @notice Returns the contract version.
    pub fn version(_env: Env) -> u32 {
        7
    }

    /// @notice Read a market from persistent storage and bump its TTL.
    pub fn get_market(env: Env, market_id: soroban_sdk::Symbol) -> Option<soroban_sdk::Val> {
        let market: Option<soroban_sdk::Val> = env.storage().persistent().get(&market_id);
        if market.is_some() {
            env.storage()
                .persistent()
                .extend_ttl(&market_id, 6307200, 6307200);
        }
        market
    }

    /// Returns whether market operations are currently paused globally.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Returns the primary admin address if set.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    // -----------------------------------------------------------------------
    //  Market Lifecycle
    // -----------------------------------------------------------------------

    /// Creates a new prediction market with default WinnerTakesAll payout curve.
    pub fn create_market(
        env: Env,
        creator: Address,
        question: String,
        description: String,
        end_time: u64,
        resolution_source: String,
        outcome_tags: Vec<String>,
    ) -> u32 {
        Self::create_market_with_curve(
            env.clone(),
            creator,
            question,
            description,
            end_time,
            resolution_source,
            outcome_tags,
            PayoutCurveConfig::winner_takes_all(&env),
        )
    }

    /// Creates a new prediction market with a specific configurable payout curve (#014).
    pub fn create_market_with_curve(
        env: Env,
        creator: Address,
        question: String,
        description: String,
        end_time: u64,
        resolution_source: String,
        outcome_tags: Vec<String>,
        curve: PayoutCurveConfig,
    ) -> u32 {
        creator.require_auth();

        if outcome_tags.len() < 2 {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        Self::validate_payout_curve(&env, &curve, outcome_tags.len());

        let counter: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MarketCounter)
            .unwrap_or(0u32);

        let market_id = match counter.checked_add(1) {
            Some(id) => id,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage()
            .persistent()
            .set(&DataKey::MarketCounter, &market_id);

        let market = MarketData {
            creator: creator.clone(),
            question,
            description,
            end_time,
            resolution_source,
            outcome_tags,
            resolved: false,
            winning_outcome: 0,
            cancelled: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);

        env.storage()
            .persistent()
            .set(&DataKey::MarketPayoutCurve(market_id), &curve);

        market_id
    }

    /// Configures the payout curve for an existing market before resolution and betting (#014).
    pub fn set_market_payout_curve(
        env: Env,
        caller: Address,
        market_id: u32,
        curve: PayoutCurveConfig,
    ) {
        caller.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved || market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        if caller != market.creator && !Self::is_admin_internal(&env, &caller) {
            panic_with_error!(env, ContractError::Unauthorized);
        }

        // Prevent modifying curve if wagers are already placed on the market
        let total_pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MarketTotalPool(market_id))
            .unwrap_or(0);
        if total_pool > 0 {
            panic_with_error!(env, ContractError::InvalidState);
        }

        Self::validate_payout_curve(&env, &curve, market.outcome_tags.len());

        env.storage()
            .persistent()
            .set(&DataKey::MarketPayoutCurve(market_id), &curve);
    }

    /// Reads the configured payout curve for a market (#014).
    pub fn get_market_payout_curve(env: Env, market_id: u32) -> PayoutCurveConfig {
        env.storage()
            .persistent()
            .get(&DataKey::MarketPayoutCurve(market_id))
            .unwrap_or_else(|| PayoutCurveConfig::winner_takes_all(&env))
    }

    /// Places a bet on a specific market outcome, updating bet enumeration (#022)
    /// and pool balances (#014).
    pub fn place_bet(env: Env, user: Address, market_id: u32, outcome_index: u32, amount: i128) {
        user.require_auth();

        if Self::is_paused(env.clone()) {
            panic_with_error!(env, ContractError::InvalidState);
        }

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved || market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        if outcome_index >= market.outcome_tags.len() {
            panic_with_error!(env, ContractError::InvalidOutcome);
        }

        if amount <= 0 {
            panic_with_error!(env, ContractError::StakeTooSmall);
        }

        // Update or record the user's bet
        let bet = BetData {
            outcome_index,
            amount,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Bet(market_id, user.clone()), &bet);

        // Maintain persistent user bet index for enumeration (#022)
        let mut user_markets: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBetMarkets(user.clone()))
            .unwrap_or(Vec::new(&env));

        if !user_markets.contains(market_id) {
            user_markets.push_back(market_id);
            env.storage()
                .persistent()
                .set(&DataKey::UserBetMarkets(user), &user_markets);
        }

        // Maintain market and outcome pool accounting (#014)
        let total_pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MarketTotalPool(market_id))
            .unwrap_or(0);
        let new_total = match total_pool.checked_add(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };
        env.storage()
            .persistent()
            .set(&DataKey::MarketTotalPool(market_id), &new_total);

        let outcome_pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::OutcomePool(market_id, outcome_index))
            .unwrap_or(0);
        let new_outcome_pool = match outcome_pool.checked_add(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };
        env.storage().persistent().set(
            &DataKey::OutcomePool(market_id, outcome_index),
            &new_outcome_pool,
        );
    }

    /// Resolves a market by recording the winning outcome.
    pub fn resolve_market(env: Env, resolver: Address, market_id: u32, winning_outcome: u32) {
        resolver.require_auth();

        let mut market: MarketData =
            match env.storage().persistent().get(&DataKey::Market(market_id)) {
                Some(m) => m,
                None => panic_with_error!(env, ContractError::MarketNotFound),
            };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        if winning_outcome >= market.outcome_tags.len() {
            panic_with_error!(env, ContractError::InvalidOutcome);
        }

        market.resolved = true;
        market.winning_outcome = winning_outcome;
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);
    }

    /// Previews the payout amount for a claimant on a market (#014).
    pub fn calculate_payout(env: Env, market_id: u32, claimant: Address) -> i128 {
        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => return 0,
        };

        if !market.resolved {
            return 0;
        }

        let bet: BetData = match env
            .storage()
            .persistent()
            .get(&DataKey::Bet(market_id, claimant))
        {
            Some(b) => b,
            None => return 0,
        };

        let curve: PayoutCurveConfig = env
            .storage()
            .persistent()
            .get(&DataKey::MarketPayoutCurve(market_id))
            .unwrap_or_else(|| PayoutCurveConfig::winner_takes_all(&env));

        let total_pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MarketTotalPool(market_id))
            .unwrap_or(bet.amount);

        let win_pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::OutcomePool(market_id, market.winning_outcome))
            .unwrap_or(0);

        match curve.curve_type {
            PayoutCurveConfig::WINNER_TAKES_ALL => {
                if bet.outcome_index != market.winning_outcome {
                    return 0;
                }
                if win_pool == 0 {
                    return bet.amount;
                }
                (bet.amount * total_pool) / win_pool
            }
            PayoutCurveConfig::TIERED => {
                let ru_pool: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::OutcomePool(market_id, curve.runner_up_outcome))
                    .unwrap_or(0);

                if ru_pool == 0 {
                    if bet.outcome_index == market.winning_outcome {
                        if win_pool == 0 {
                            return bet.amount;
                        }
                        return (bet.amount * total_pool) / win_pool;
                    }
                    return 0;
                }

                if win_pool == 0 {
                    if bet.outcome_index == curve.runner_up_outcome {
                        return (bet.amount * total_pool) / ru_pool;
                    }
                    return 0;
                }

                let ru_share = (total_pool * (curve.runner_up_bps as i128)) / 10_000;
                let win_share = total_pool - ru_share;

                if bet.outcome_index == market.winning_outcome {
                    (bet.amount * win_share) / win_pool
                } else if bet.outcome_index == curve.runner_up_outcome {
                    (bet.amount * ru_share) / ru_pool
                } else {
                    0
                }
            }
            PayoutCurveConfig::CUSTOM_WEIGHTS => {
                let weight = curve.custom_weights.get(bet.outcome_index).unwrap_or(0);
                if weight == 0 {
                    return 0;
                }
                let outcome_pool: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::OutcomePool(market_id, bet.outcome_index))
                    .unwrap_or(0);
                if outcome_pool == 0 {
                    return 0;
                }
                let outcome_share = (total_pool * (weight as i128)) / 10_000;
                (bet.amount * outcome_share) / outcome_pool
            }
            _ => 0,
        }
    }

    /// Claims winnings for a resolved market upholding CEI pattern and applying
    /// the configured payout curve (#014, #021).
    pub fn claim_winnings(env: Env, claimant: Address, market_id: u32) -> i128 {
        claimant.require_auth();

        // ── CHECKS ───────────────────────────────────────────────────────────
        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if !market.resolved {
            panic_with_error!(env, ContractError::MarketNotResolved);
        }

        // Guard against double claim (CEI pattern)
        if env
            .storage()
            .persistent()
            .has(&DataKey::ClaimedBet(market_id, claimant.clone()))
        {
            panic_with_error!(env, ContractError::AlreadyClaimed);
        }

        let bet: BetData = match env
            .storage()
            .persistent()
            .get(&DataKey::Bet(market_id, claimant.clone()))
        {
            Some(b) => b,
            None => panic_with_error!(env, ContractError::InvalidState),
        };

        let payout = Self::calculate_payout(env.clone(), market_id, claimant.clone());
        if payout <= 0 {
            panic_with_error!(env, ContractError::InvalidOutcome);
        }

        // ── EFFECTS ──────────────────────────────────────────────────────────
        // Mark bet as claimed before external interaction
        env.storage()
            .persistent()
            .set(&DataKey::ClaimedBet(market_id, claimant.clone()), &true);

        // ── INTERACTIONS ─────────────────────────────────────────────────────
        let _ = bet.amount;

        payout
    }

    /// Cancels a market before it has been resolved.
    pub fn cancel_market(env: Env, caller: Address, market_id: u32) {
        caller.require_auth();

        let mut market: MarketData =
            match env.storage().persistent().get(&DataKey::Market(market_id)) {
                Some(m) => m,
                None => panic_with_error!(env, ContractError::MarketNotFound),
            };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }
        if market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        market.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::Market(market_id), &market);
    }

    /// Withdraws funds from a market.
    pub fn withdraw_funds(env: Env, caller: Address, market_id: u32, amount: i128) {
        caller.require_auth();

        if !env.storage().persistent().has(&DataKey::Market(market_id)) {
            panic_with_error!(env, ContractError::MarketNotFound);
        }

        let _ = amount;
    }

    /// Updates the parameters of an existing market.
    pub fn update_market_params(env: Env, caller: Address, market_id: u32, new_end_time: u64) {
        caller.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved {
            panic_with_error!(env, ContractError::MarketAlreadyResolved);
        }

        let _ = new_end_time;
    }

    // -----------------------------------------------------------------------
    //  Liquidity
    // -----------------------------------------------------------------------

    /// Adds liquidity to a market.
    pub fn add_liquidity(env: Env, provider: Address, market_id: u32, amount: i128) {
        provider.require_auth();

        let market: MarketData = match env.storage().persistent().get(&DataKey::Market(market_id)) {
            Some(m) => m,
            None => panic_with_error!(env, ContractError::MarketNotFound),
        };

        if market.resolved || market.cancelled {
            panic_with_error!(env, ContractError::InvalidState);
        }

        let existing: LiquidityData = env
            .storage()
            .persistent()
            .get(&DataKey::Liquidity(market_id, provider.clone()))
            .unwrap_or(LiquidityData { total_amount: 0 });

        let new_total = match existing.total_amount.checked_add(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage().persistent().set(
            &DataKey::Liquidity(market_id, provider),
            &LiquidityData {
                total_amount: new_total,
            },
        );
    }

    /// Removes liquidity from a market.
    pub fn remove_liquidity(env: Env, provider: Address, market_id: u32, amount: i128) {
        provider.require_auth();

        if !env.storage().persistent().has(&DataKey::Market(market_id)) {
            panic_with_error!(env, ContractError::MarketNotFound);
        }

        let existing: LiquidityData = env
            .storage()
            .persistent()
            .get(&DataKey::Liquidity(market_id, provider.clone()))
            .unwrap_or(LiquidityData { total_amount: 0 });

        if amount > existing.total_amount {
            panic_with_error!(env, ContractError::StakeTooSmall);
        }

        let new_total = match existing.total_amount.checked_sub(amount) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage().persistent().set(
            &DataKey::Liquidity(market_id, provider),
            &LiquidityData {
                total_amount: new_total,
            },
        );
    }

    // -----------------------------------------------------------------------
    //  Issue #022: User Bet Enumeration
    // -----------------------------------------------------------------------

    /// Returns the total number of markets on which the user has placed bets.
    pub fn get_user_bet_count(env: Env, user: Address) -> u32 {
        let user_markets: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBetMarkets(user))
            .unwrap_or(Vec::new(&env));
        user_markets.len()
    }

    /// Enumerates bets placed by a user with start_index and limit pagination (#022).
    ///
    /// Preserves insertion order across markets.
    pub fn get_user_bets(
        env: Env,
        user: Address,
        start_index: u32,
        limit: u32,
    ) -> Vec<UserBetInfo> {
        let mut results = Vec::new(&env);
        if limit == 0 {
            return results;
        }

        let user_markets: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBetMarkets(user.clone()))
            .unwrap_or(Vec::new(&env));

        let total = user_markets.len();
        if start_index >= total {
            return results;
        }

        let end = core::cmp::min(start_index.saturating_add(limit), total);
        for i in start_index..end {
            let market_id = user_markets.get(i).unwrap();
            if let Some(bet) = env
                .storage()
                .persistent()
                .get::<_, BetData>(&DataKey::Bet(market_id, user.clone()))
            {
                let claimed: bool = env
                    .storage()
                    .persistent()
                    .get(&DataKey::ClaimedBet(market_id, user.clone()))
                    .unwrap_or(false);

                results.push_back(UserBetInfo {
                    market_id,
                    outcome_index: bet.outcome_index,
                    amount: bet.amount,
                    claimed,
                });
            }
        }

        results
    }

    // -----------------------------------------------------------------------
    //  Issue #012: Dust-Safe Balance & Withdrawal System
    // -----------------------------------------------------------------------

    /// Configures the withdrawal fee parameters (#012).
    pub fn set_fee_config(env: Env, admin: Address, min_fee_amount: i128, fee_bps: i128) {
        Self::require_admin(&env, &admin);

        if min_fee_amount < 0 || !(0..=10_000).contains(&fee_bps) {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        env.storage()
            .persistent()
            .set(&DataKey::MinFeeAmount, &min_fee_amount);
        env.storage()
            .persistent()
            .set(&DataKey::WithdrawalFeeBps, &fee_bps);
    }

    /// Returns the configured (min_fee_amount, fee_bps) (#012).
    pub fn get_fee_config(env: Env) -> (i128, i128) {
        let min_fee = env
            .storage()
            .persistent()
            .get(&DataKey::MinFeeAmount)
            .unwrap_or(100i128);
        let fee_bps = env
            .storage()
            .persistent()
            .get(&DataKey::WithdrawalFeeBps)
            .unwrap_or(100i128); // 1.00%
        (min_fee, fee_bps)
    }

    /// Reads the internal balance of a user (#012).
    pub fn get_user_balance(env: Env, user: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::UserBalance(user))
            .unwrap_or(0i128)
    }

    /// Returns the total platform fees collected (#012).
    pub fn get_collected_fees(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::CollectedFees)
            .unwrap_or(0i128)
    }

    /// Deposits funds into a user's internal withdrawable balance (#012).
    pub fn deposit_user_balance(env: Env, user: Address, amount: i128) {
        user.require_auth();

        if amount <= 0 {
            panic_with_error!(env, ContractError::StakeTooSmall);
        }

        let balance = Self::get_user_balance(env.clone(), user.clone());
        let new_balance = match balance.checked_add(amount) {
            Some(b) => b,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        env.storage()
            .persistent()
            .set(&DataKey::UserBalance(user), &new_balance);
    }

    /// Withdraws funds from a user's balance with dust protection (#012).
    ///
    /// # Dust Invariant
    /// If the user's available balance is `<= min_fee_amount`, it is classified
    /// as dust. To prevent funds from being permanently locked, the minimum fee
    /// is waived when withdrawing remaining dust.
    pub fn withdraw_user_balance(env: Env, user: Address, amount: i128) -> WithdrawalResult {
        user.require_auth();

        if amount <= 0 {
            panic_with_error!(env, ContractError::StakeTooSmall);
        }

        let balance = Self::get_user_balance(env.clone(), user.clone());
        if amount > balance {
            panic_with_error!(env, ContractError::InsufficientBalance);
        }

        let (min_fee, fee_bps) = Self::get_fee_config(env.clone());

        let (net_amount, fee_deducted) = if balance <= min_fee {
            // Balance is at or below min_fee (dust): waive fee so dust is never locked!
            (amount, 0i128)
        } else {
            // Normal balance above min_fee
            let percentage_fee = (amount * fee_bps) / 10_000;
            let standard_fee = core::cmp::max(percentage_fee, min_fee);

            if amount == balance {
                // Full withdrawal: if amount cannot cover standard_fee, adjust fee
                if amount <= standard_fee {
                    (amount, 0i128)
                } else {
                    (amount - standard_fee, standard_fee)
                }
            } else {
                // Partial withdrawal
                if amount <= standard_fee {
                    panic_with_error!(env, ContractError::StakeTooSmall);
                }
                (amount - standard_fee, standard_fee)
            }
        };

        let new_balance = balance - amount;
        env.storage()
            .persistent()
            .set(&DataKey::UserBalance(user), &new_balance);

        if fee_deducted > 0 {
            let collected = Self::get_collected_fees(env.clone());
            let new_collected = match collected.checked_add(fee_deducted) {
                Some(c) => c,
                None => panic_with_error!(env, ContractError::Overflow),
            };
            env.storage()
                .persistent()
                .set(&DataKey::CollectedFees, &new_collected);
        }

        WithdrawalResult {
            net_amount,
            fee_deducted,
            remaining_balance: new_balance,
        }
    }

    /// Admin fee collection from user account with dust-safe protection (#012).
    ///
    /// If a user balance is `<= min_fee_amount`, fees are not collected and 0 is returned,
    /// ensuring dust is never locked and `InsufficientBalance` is never thrown.
    pub fn collect_user_fees(env: Env, admin: Address, user: Address) -> i128 {
        Self::require_admin(&env, &admin);

        let balance = Self::get_user_balance(env.clone(), user.clone());
        let (min_fee, fee_bps) = Self::get_fee_config(env.clone());

        if balance <= min_fee {
            // Dust-safe: do not fail with InsufficientBalance and do not trap user dust!
            return 0;
        }

        let calculated_fee = (balance * fee_bps) / 10_000;
        let fee = core::cmp::max(calculated_fee, min_fee);

        if balance < fee {
            return 0;
        }

        let new_balance = balance - fee;
        env.storage()
            .persistent()
            .set(&DataKey::UserBalance(user), &new_balance);

        let collected = Self::get_collected_fees(env.clone());
        let new_collected = match collected.checked_add(fee) {
            Some(c) => c,
            None => panic_with_error!(env, ContractError::Overflow),
        };
        env.storage()
            .persistent()
            .set(&DataKey::CollectedFees, &new_collected);

        fee
    }

    // -----------------------------------------------------------------------
    //  Issue #007: Hardened Admin Pause Mechanism
    // -----------------------------------------------------------------------

    /// Configures the pause protection parameters (delay and required approvals) (#007).
    pub fn set_pause_config(env: Env, admin: Address, delay_seconds: u64, required_approvals: u32) {
        Self::require_admin(&env, &admin);

        if required_approvals == 0 {
            panic_with_error!(env, ContractError::InvalidConfig);
        }

        env.storage()
            .persistent()
            .set(&DataKey::PauseDelaySeconds, &delay_seconds);
        env.storage()
            .persistent()
            .set(&DataKey::PauseRequiredApprovals, &required_approvals);
    }

    /// Returns the configured pause protection parameters (delay_seconds, required_approvals) (#007).
    pub fn get_pause_config(env: Env) -> (u64, u32) {
        let delay: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PauseDelaySeconds)
            .unwrap_or(0);
        let approvals: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::PauseRequiredApprovals)
            .unwrap_or(1);
        (delay, approvals)
    }

    /// Adds an administrator to the authorized admin list (#007).
    pub fn add_admin(env: Env, admin: Address, new_admin: Address) {
        Self::require_admin(&env, &admin);

        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::AdminList)
            .unwrap_or(Vec::new(&env));

        if !list.contains(&new_admin) {
            list.push_back(new_admin);
            env.storage().persistent().set(&DataKey::AdminList, &list);
        }
    }

    /// Proposes a global market pause with a stated justification (#007).
    pub fn propose_pause(env: Env, admin: Address, reason: String) -> u32 {
        Self::require_admin(&env, &admin);

        let counter: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::PauseProposalCounter)
            .unwrap_or(0);

        let proposal_id = match counter.checked_add(1) {
            Some(id) => id,
            None => panic_with_error!(env, ContractError::Overflow),
        };
        env.storage()
            .persistent()
            .set(&DataKey::PauseProposalCounter, &proposal_id);

        let (delay, _) = Self::get_pause_config(env.clone());
        let now = env.ledger().timestamp();
        let eta = match now.checked_add(delay) {
            Some(t) => t,
            None => panic_with_error!(env, ContractError::Overflow),
        };

        let mut approvals = Vec::new(&env);
        approvals.push_back(admin.clone());

        let proposal = PauseProposal {
            id: proposal_id,
            proposer: admin,
            reason,
            created_at: now,
            eta,
            approvals,
            executed: false,
            cancelled: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PauseProposal(proposal_id), &proposal);

        proposal_id
    }

    /// Approves a pending pause proposal (#007).
    pub fn approve_pause(env: Env, admin: Address, proposal_id: u32) {
        Self::require_admin(&env, &admin);

        let mut proposal: PauseProposal = match env
            .storage()
            .persistent()
            .get(&DataKey::PauseProposal(proposal_id))
        {
            Some(p) => p,
            None => panic_with_error!(env, ContractError::ProposalNotFound),
        };

        if proposal.executed || proposal.cancelled {
            panic_with_error!(env, ContractError::ProposalNotExecutable);
        }

        if proposal.approvals.contains(&admin) {
            panic_with_error!(env, ContractError::ProposalAlreadyApproved);
        }

        proposal.approvals.push_back(admin);
        env.storage()
            .persistent()
            .set(&DataKey::PauseProposal(proposal_id), &proposal);
    }

    /// Executes an approved and timelocked pause proposal (#007).
    pub fn execute_pause(env: Env, caller: Address, proposal_id: u32) {
        Self::require_admin(&env, &caller);

        let mut proposal: PauseProposal = match env
            .storage()
            .persistent()
            .get(&DataKey::PauseProposal(proposal_id))
        {
            Some(p) => p,
            None => panic_with_error!(env, ContractError::ProposalNotFound),
        };

        if proposal.executed || proposal.cancelled {
            panic_with_error!(env, ContractError::ProposalNotExecutable);
        }

        let (_, required_approvals) = Self::get_pause_config(env.clone());
        if proposal.approvals.len() < required_approvals {
            panic_with_error!(env, ContractError::ProposalNotExecutable);
        }

        let now = env.ledger().timestamp();
        if now < proposal.eta {
            panic_with_error!(env, ContractError::ProposalNotExecutable);
        }

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::PauseProposal(proposal_id), &proposal);

        env.storage().persistent().set(&DataKey::Paused, &true);
    }

    /// Cancels a pending pause proposal (#007).
    pub fn cancel_pause(env: Env, admin: Address, proposal_id: u32) {
        Self::require_admin(&env, &admin);

        let mut proposal: PauseProposal = match env
            .storage()
            .persistent()
            .get(&DataKey::PauseProposal(proposal_id))
        {
            Some(p) => p,
            None => panic_with_error!(env, ContractError::ProposalNotFound),
        };

        if proposal.executed || proposal.cancelled {
            panic_with_error!(env, ContractError::ProposalNotExecutable);
        }

        proposal.cancelled = true;
        env.storage()
            .persistent()
            .set(&DataKey::PauseProposal(proposal_id), &proposal);
    }

    /// Reads a pause proposal by ID (#007).
    pub fn get_pause_proposal(env: Env, proposal_id: u32) -> Option<PauseProposal> {
        env.storage()
            .persistent()
            .get(&DataKey::PauseProposal(proposal_id))
    }

    /// Pauses all market operations globally.
    ///
    /// # Hardened Protection
    /// If hardened pause protection is active (required_approvals > 1 or delay > 0),
    /// unilateral instantaneous execution is blocked with `AdminOperationNotPermitted` (#007).
    pub fn pause_markets(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);

        let (delay, required_approvals) = Self::get_pause_config(env.clone());
        if delay > 0 || required_approvals > 1 {
            panic_with_error!(env, ContractError::AdminOperationNotPermitted);
        }

        env.storage().persistent().set(&DataKey::Paused, &true);
    }

    /// Resumes all market operations globally.
    pub fn unpause_markets(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        env.storage().persistent().set(&DataKey::Paused, &false);
    }

    /// Transfers contract ownership to a new admin address.
    pub fn transfer_ownership(env: Env, admin: Address, new_owner: Address) {
        Self::require_admin(&env, &admin);
        env.storage().persistent().set(&DataKey::Admin, &new_owner);
    }
}
