//! Types and schemas for the Analytics contract.

use soroban_sdk::contracttype;

/// A single time-bucketed aggregation result for charting and time-series visualization.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeBucket {
    /// Bucket start timestamp in seconds (inclusive, epoch-aligned).
    pub timestamp: u64,
    /// Number of data points recorded in this window.
    pub count: u32,
    /// Sum of all values in this window.
    pub sum: u64,
    /// Minimum value in this window.
    pub min: u64,
    /// Maximum value in this window.
    pub max: u64,
    /// First recorded value in this window (open).
    pub open: u64,
    /// Last recorded value in this window (close).
    pub close: u64,
    /// Average (mean) value in this window (`sum / count`).
    pub avg: u64,
}

/// An individual timestamped data point.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataPoint {
    /// Ledger timestamp when the data point was recorded.
    pub timestamp: u64,
    /// Numeric value of the data point.
    pub value: u64,
}

/// Cumulative summary statistics for a tracked metric.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricSummary {
    /// Total number of recorded points.
    pub total_count: u32,
    /// Sum of all recorded values.
    pub total_sum: u64,
    /// Minimum recorded value across all time.
    pub min_value: u64,
    /// Maximum recorded value across all time.
    pub max_value: u64,
    /// Most recent recorded value.
    pub latest_value: u64,
    /// Timestamp of first recorded value.
    pub first_timestamp: u64,
    /// Timestamp of most recent recorded value.
    pub latest_timestamp: u64,
}

/// Predefined standard time aggregation windows (in seconds).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum TimeWindow {
    /// 1 minute (60 seconds)
    OneMinute = 60,
    /// 5 minutes (300 seconds)
    FiveMinutes = 300,
    /// 15 minutes (900 seconds)
    FifteenMinutes = 900,
    /// 1 hour (3,600 seconds)
    OneHour = 3600,
    /// 4 hours (14,400 seconds)
    FourHours = 14400,
    /// 1 day (86,400 seconds)
    OneDay = 86400,
    /// 1 week (604,800 seconds)
    OneWeek = 604800,
    /// 30 days (2,592,000 seconds)
    ThirtyDays = 2592000,
}

impl TimeWindow {
    /// Returns the window size in seconds.
    pub fn seconds(&self) -> u64 {
        match self {
            TimeWindow::OneMinute => 60,
            TimeWindow::FiveMinutes => 300,
            TimeWindow::FifteenMinutes => 900,
            TimeWindow::OneHour => 3600,
            TimeWindow::FourHours => 14400,
            TimeWindow::OneDay => 86400,
            TimeWindow::OneWeek => 604800,
            TimeWindow::ThirtyDays => 2592000,
        }
    }
}
