#![allow(dead_code)]

//! Unified type exports for the Predictify Hybrid prediction market contract.
//!
//! This module re-exports all types from domain-specific type modules, maintaining
//! backward compatibility with existing code while enabling cleaner organization.
//!
//! # Module Structure
//!
//! Types are organized into the following domain-specific modules:
//!
//! - **oracle_types**: Oracle configuration and pricing (15 types)
//! - **market_types**: Market definition and lifecycle (7 types)
//! - **bet_types**: Betting operations (4 types)
//! - **dispute_types**: Disputes and events (3 types)
//! - **query_types**: Query response structures (9 types)
//! - **support_types**: Supporting utilities (4 types)
//! - **statistics_types**: Analytics and metrics (8 types)
//! - **fee_types**: Fee structures (reserved for future use)
//! - **governance_types**: Governance structures (reserved for future use)
//!
//! Total: 49 types across 9 domain modules.
//!
//! # Usage
//!
//! ```rust
//! use predictify_hybrid::types::{Market, OracleConfig, Bet};
//! ```
//!
//! All types are available at the top level for backward compatibility.

// ===== RE-EXPORTS: ORACLE TYPES =====

pub use crate::oracle_types::{
    EventOracleValidationConfig, GlobalOracleValidationConfig, MedianOracleConfig,
    MultiOracleResult, OracleConfig, OracleFetchRequest, OraclePriceData, OracleProvider,
    OracleQuote, OracleResult, OracleSource, OracleVerificationStatus, ReflectorAsset,
    ReflectorPriceData,
};

// ===== RE-EXPORTS: MARKET TYPES =====

pub use crate::market_types::{
    CommunityConsensus, Market, MarketMetadataCommitmentPayload, MarketPauseInfo,
    MarketState, MarketStatus, PauseReason,
};

// ===== RE-EXPORTS: BET TYPES =====

pub use crate::bet_types::{Bet, BetLimits, BetStats, BetStatus};

// ===== RE-EXPORTS: DISPUTE & EVENT TYPES =====

pub use crate::dispute_types::{Event, EventHistoryEntry, EventVisibility};

// ===== RE-EXPORTS: QUERY TYPES =====

pub use crate::query_types::{
    ContractStateQuery, EventDetailsQuery, MarketPoolQuery, MultipleBetsQuery,
    PagedMarketIds, PagedUserBets, UserBalanceQuery, UserBetPagedResult, UserBetQuery,
};

// ===== RE-EXPORTS: SUPPORT TYPES =====

pub use crate::support_types::{Balance, ClaimInfo, MarketCreationParams, MarketExtension};

// ===== RE-EXPORTS: STATISTICS TYPES =====

pub use crate::statistics_types::{
    CategoryStatisticsV1, DashboardStatisticsV1, ExtensionStats, MarketLeaderboardEntry,
    MarketStatisticsV1, PlatformStatistics, UserLeaderboardEntryV1, UserStatistics,
};
