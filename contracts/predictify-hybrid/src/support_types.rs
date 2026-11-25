#![allow(dead_code)]

use crate::oracle_types::OracleConfig;
use soroban_sdk::{contracttype, Address, Env, String};

// ===== CLAIM INFO =====

/// Claim information for tracking idempotent winnings claims.
///
/// This struct stores comprehensive information about a user's claim,
/// enabling idempotency checks, audit trails, and safe retry mechanisms.
///
/// # Fields
///
/// - `claimed`: Whether the user has claimed their winnings
/// - `timestamp`: Ledger timestamp when the claim was processed (Unix timestamp)
/// - `payout_amount`: The exact amount of tokens claimed (for verification and audits)
///
/// # Security Properties
///
/// - **Immutable**: Once set, the claim record cannot be modified (append-only)
/// - **Idempotent**: Multiple claim attempts return the same result without side effects
/// - **Verifiable**: Payout amount can be verified against actual transfer
/// - **Auditable**: Timestamp provides proof of when claim occurred
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimInfo {
    /// Whether the user has claimed their winnings
    pub claimed: bool,
    /// Ledger timestamp when the claim was processed (Unix timestamp)
    pub timestamp: u64,
    /// The exact amount of tokens claimed (for verification and audits)
    pub payout_amount: i128,
}

impl ClaimInfo {
    /// Create a new ClaimInfo instance with the given payout amount.
    ///
    /// # Parameters
    ///
    /// - `env` - The Soroban environment (used for timestamp)
    /// - `payout_amount` - The amount being claimed
    ///
    /// # Returns
    ///
    /// Returns a new ClaimInfo with current timestamp and specified payout.
    pub fn new(env: &Env, payout_amount: i128) -> Self {
        Self {
            claimed: true,
            timestamp: env.ledger().timestamp(),
            payout_amount,
        }
    }

    /// Create a default (unclaimed) ClaimInfo instance.
    ///
    /// # Returns
    ///
    /// Returns a ClaimInfo with claimed=false and zero values.
    pub fn unclaimed() -> Self {
        Self {
            claimed: false,
            timestamp: 0,
            payout_amount: 0,
        }
    }

    /// Check if this claim has been processed.
    ///
    /// # Returns
    ///
    /// Returns `true` if claimed, `false` otherwise.
    pub fn is_claimed(&self) -> bool {
        self.claimed
    }

    /// Get the timestamp when this claim was processed.
    ///
    /// # Returns
    ///
    /// Returns the ledger timestamp as Unix timestamp in seconds.
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get the payout amount that was claimed.
    ///
    /// # Returns
    ///
    /// Returns the exact token amount claimed (in stroops/smallest unit).
    pub fn get_payout(&self) -> i128 {
        self.payout_amount
    }
}

// ===== MARKET CREATION TYPES =====

/// Comprehensive parameters for creating new prediction markets.
///
/// This structure contains all necessary information to create a new prediction
/// market, including administrative details, market configuration, oracle setup,
/// and financial requirements. It serves as the complete specification for
/// market initialization and validation.
///
/// # Parameter Categories
///
/// **Administrative Setup:**
/// - **Admin**: Market administrator with management privileges
/// - **Creation Fee**: Cost to create the market
///
/// **Market Definition:**
/// - **Question**: The prediction question being resolved
/// - **Outcomes**: Available outcomes users can vote on
/// - **Duration**: How long the market remains active
///
/// **Oracle Integration:**
/// - **Oracle Config**: Configuration for automated resolution
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketCreationParams {
    /// Market administrator address
    pub admin: Address,
    /// Market question/prediction
    pub question: String,
    /// Available outcomes for the market
    pub outcomes: soroban_sdk::Vec<String>,
    /// Market duration in days
    pub duration_days: u32,
    /// Oracle configuration for this market
    pub oracle_config: OracleConfig,
    /// Creation fee amount
    pub creation_fee: i128,
    /// Per-market dispute stake floor (None = use global/default minimum)
    pub dispute_stake_floor: Option<i128>,
}

impl MarketCreationParams {
    /// Create new market creation parameters
    pub fn new(
        admin: Address,
        question: String,
        outcomes: soroban_sdk::Vec<String>,
        duration_days: u32,
        oracle_config: OracleConfig,
        creation_fee: i128,
        dispute_stake_floor: Option<i128>,
    ) -> Self {
        Self {
            admin,
            question,
            outcomes,
            duration_days,
            oracle_config,
            creation_fee,
            dispute_stake_floor,
        }
    }
}

// ===== MARKET EXTENSION TYPES =====

/// Market extension data structure for time-based market lifecycle management.
///
/// This structure manages the extension of market voting periods, allowing markets
/// to have their end times adjusted under specific conditions. Extensions provide
/// flexibility for markets that may need additional time due to low participation,
/// significant events, or community requests.
///
/// # Extension Components
///
/// **Extension Request:**
/// - **Requester**: Address that requested the extension
/// - **Original End Time**: Market's initial end time
/// - **New End Time**: Proposed new end time after extension
/// - **Extension Duration**: Length of the extension in seconds
///
/// **Extension Justification:**
/// - **Reason**: Explanation for why extension is needed
/// - **Fee**: Cost paid for the extension request
/// - **Approval Status**: Whether extension has been approved
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketExtension {
    /// Number of additional days
    pub additional_days: u32,
    /// Administrator who requested the extension
    pub admin: Address,
    /// Reason for the extension
    pub reason: String,
    /// Fee amount paid
    pub fee_amount: i128,
    /// Extension timestamp
    pub timestamp: u64,
}

impl MarketExtension {
    /// Create a new market extension
    pub fn new(
        env: &Env,
        additional_days: u32,
        admin: Address,
        reason: String,
        fee_amount: i128,
    ) -> Self {
        Self {
            additional_days,
            admin,
            reason,
            fee_amount,
            timestamp: env.ledger().timestamp(),
        }
    }

    /// Validate the market extension
    pub fn validate(&self) -> Result<(), crate::Error> {
        // Validate reason length
        crate::metadata_limits::validate_extension_reason_length(&self.reason)?;

        Ok(())
    }
}

// ===== BALANCE TRACKING =====

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Balance {
    pub user: Address,
    pub asset: crate::oracle_types::ReflectorAsset,
    pub amount: i128,
}
