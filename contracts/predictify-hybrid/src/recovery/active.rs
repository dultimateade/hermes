//! Live recovery state and per-market timelock handling.

pub use super::{
    DryRunResult, MarketRecovery, PendingMarketRecovery, PerMarketRecoveryAction,
    RecoveryData, RecoveryManager, RecoveryTimelockConfig, RecoveryValidator,
};

pub const ACTIVE_RECOVERY_TTL_LEDGERS: u32 = 365 * 17_280;
pub const ACTIVE_RECOVERY_LIFETIME_THRESHOLD: u32 = 31 * 17_280;
