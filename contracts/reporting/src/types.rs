//! Storage keys and data types for the Reporting contract.

use soroban_sdk::{contracttype, Address, String};

/// Metadata for a submitted report.
#[contracttype]
pub struct Report {
    /// Unique report identifier.
    pub id: u32,
    /// Market this report relates to.
    pub market_id: u32,
    /// Address of the reporter who submitted this report.
    pub reporter: Address,
    /// Human-readable or structured report payload.
    pub report_data: String,
    /// On-chain commitment hash (e.g., SHA-256 hex string).
    pub report_hash: String,
    /// Numeric status code (e.g., 0 = pending, 1 = verified, 2 = disputed).
    pub status: u32,
    /// Block height at which this report was submitted.
    pub created_at: u32,
}

/// Persistent-storage keys used by the Reporting contract.
#[contracttype]
pub enum DataKey {
    /// Whether the contract has been initialized.
    Initialized,
    /// The current admin address.
    Admin,
    /// Whether reporting is paused (`true` = paused).
    ReportingPaused,
    /// Next report ID counter.
    NextReportId,
    /// Next dispute ID counter.
    NextDisputeId,
    /// Report by ID: `DataKey::Report(id)` → `Report`
    Report(u32),
    /// List of report IDs for a market: `DataKey::MarketReports(market_id)` → Vec<u32>
    MarketReports(u32),
}
