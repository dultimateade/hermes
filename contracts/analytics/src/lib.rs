//! On-chain time-series analytics for global and market-specific metrics.
//!
//! Each metric stream retains timestamped numeric data points and a cumulative
//! summary. The default capacity is 10,000 points per stream and the admin can
//! change it. Capacity is not an age-based retention period: this contract does
//! not prune or replace old points, and recording fails when a stream is full.
//! Persistent entries remain subject to Soroban storage TTL and archival rules.
//!
//! Queries provide cumulative summaries for global and market metrics, raw
//! points for global metrics over an inclusive time range, and populated time
//! buckets for global or market metrics. Bucket windows range from 1 second to
//! 1 year, with at most 2,000 estimated buckets per query. Raw-point queries
//! accept a result limit; a limit of zero returns all matching points.

#![no_std]

mod errors;
pub mod storage;
pub mod types;

pub use errors::ContractError;
pub use storage::DataKey;
pub use types::{DataPoint, MetricSummary, TimeBucket, TimeWindow};

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Env, Symbol, Vec,
};

/// Maximum distance between a supplied metric timestamp and ledger time.
/// Adjust this deployment constant when the expected ingestion delay changes.
const MAX_TIMESTAMP_DRIFT_SECONDS: u64 = 86_400;

#[contract]
pub struct Analytics;

#[contractimpl]
impl Analytics {
    // =========================================================================
    // Initialization & Administration
    // =========================================================================

    /// Initializes the analytics contract with the specified admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::MaxCapacity, &storage::DEFAULT_MAX_CAPACITY);

        env.events().publish(
            (symbol_short!("analytics"), symbol_short!("init")),
            admin,
        );

        Ok(())
    }

    /// Sets a new admin for the contract. Requires authorization from current admin.
    pub fn set_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), ContractError> {
        Self::assert_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::Admin, &new_admin);

        env.events().publish(
            (symbol_short!("analytics"), symbol_short!("set_admin")),
            (caller, new_admin),
        );

        Ok(())
    }

    /// Returns the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)
    }

    /// Pauses or unpauses analytics recording and querying.
    pub fn set_paused(env: Env, caller: Address, paused: bool) -> Result<(), ContractError> {
        Self::assert_admin(&env, &caller)?;
        env.storage().instance().set(&DataKey::Paused, &paused);

        env.events().publish(
            (symbol_short!("analytics"), symbol_short!("pause")),
            paused,
        );

        Ok(())
    }

    /// Checks whether analytics is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Sets the maximum capacity of data points stored per metric stream.
    pub fn set_capacity(env: Env, caller: Address, max_capacity: u32) -> Result<(), ContractError> {
        Self::assert_admin(&env, &caller)?;
        if max_capacity == 0 {
            return Err(ContractError::InvalidConfig);
        }
        env.storage()
            .instance()
            .set(&DataKey::MaxCapacity, &max_capacity);
        Ok(())
    }

    // =========================================================================
    // Time-Series & Bucketed Analytics Endpoints
    // =========================================================================

    /// Record a metric data point at current ledger timestamp.
    pub fn record_metric(env: Env, metric: Symbol, value: u64) -> Result<u64, ContractError> {
        let timestamp = env.ledger().timestamp();
        Self::record_metric_at(env, metric, value, timestamp)
    }

    /// Record a metric data point at a specific timestamp.
    ///
    /// The supplied timestamp must be within [`MAX_TIMESTAMP_DRIFT_SECONDS`] of
    /// the current ledger time; timestamps outside that window are rejected with
    /// [`ContractError::ValueOutOfRange`].
    pub fn record_metric_at(
        env: Env,
        metric: Symbol,
        value: u64,
        timestamp: u64,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;

        let current_timestamp = env.ledger().timestamp();
        if timestamp.abs_diff(current_timestamp) > MAX_TIMESTAMP_DRIFT_SECONDS {
            return Err(ContractError::ValueOutOfRange);
        }

        let key = DataKey::MetricPoints(metric.clone());
        let summary_key = DataKey::MetricSummary(metric.clone());
        Self::store_point(&env, &key, &summary_key, value, timestamp)?;

        env.events().publish(
            (symbol_short!("analytics"), symbol_short!("record"), metric),
            (value, timestamp),
        );

        Ok(value)
    }

    /// Record a market-specific metric data point at current ledger timestamp.
    pub fn record_market_metric(
        env: Env,
        market_id: Symbol,
        metric: Symbol,
        value: u64,
    ) -> Result<u64, ContractError> {
        let timestamp = env.ledger().timestamp();
        Self::record_market_metric_at(env, market_id, metric, value, timestamp)
    }

    /// Record a market-specific metric data point at a specific timestamp.
    ///
    /// The supplied timestamp must be within [`MAX_TIMESTAMP_DRIFT_SECONDS`] of
    /// the current ledger time; timestamps outside that window are rejected with
    /// [`ContractError::ValueOutOfRange`].
    pub fn record_market_metric_at(
        env: Env,
        market_id: Symbol,
        metric: Symbol,
        value: u64,
        timestamp: u64,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;

        let current_timestamp = env.ledger().timestamp();
        if timestamp.abs_diff(current_timestamp) > MAX_TIMESTAMP_DRIFT_SECONDS {
            return Err(ContractError::ValueOutOfRange);
        }

        let key = DataKey::MarketMetricPoints(market_id.clone(), metric.clone());
        let summary_key = DataKey::MarketMetricSummary(market_id.clone(), metric.clone());
        Self::store_point(&env, &key, &summary_key, value, timestamp)?;

        env.events().publish(
            (
                symbol_short!("analytics"),
                symbol_short!("m_record"),
                market_id,
            ),
            (metric, value, timestamp),
        );

        Ok(value)
    }

    /// Returns time-bucketed aggregation for a global metric over `[start_time, end_time]`
    /// partitioned into discrete buckets of duration `window_size` (in seconds).
    ///
    /// For charting, each bucket contains: `timestamp` (bucket start), `count`, `sum`,
    /// `min`, `max`, `open`, `close`, `avg`.
    pub fn get_time_series(
        env: Env,
        metric: Symbol,
        start_time: u64,
        end_time: u64,
        window_size: u64,
    ) -> Result<Vec<TimeBucket>, ContractError> {
        Self::assert_not_paused(&env)?;
        Self::validate_time_query(start_time, end_time, window_size)?;

        let key = DataKey::MetricPoints(metric);
        let points: Vec<DataPoint> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        Self::compute_time_buckets(&env, &points, start_time, end_time, window_size)
    }

    /// Returns time-bucketed aggregation for a market-specific metric over `[start_time, end_time]`.
    pub fn get_market_time_series(
        env: Env,
        market_id: Symbol,
        metric: Symbol,
        start_time: u64,
        end_time: u64,
        window_size: u64,
    ) -> Result<Vec<TimeBucket>, ContractError> {
        Self::assert_not_paused(&env)?;
        Self::validate_time_query(start_time, end_time, window_size)?;

        let key = DataKey::MarketMetricPoints(market_id, metric);
        let points: Vec<DataPoint> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        Self::compute_time_buckets(&env, &points, start_time, end_time, window_size)
    }

    /// Returns time-bucketed aggregation using a standard `TimeWindow` enum variant.
    pub fn get_time_buckets(
        env: Env,
        metric: Symbol,
        start_time: u64,
        end_time: u64,
        window: TimeWindow,
    ) -> Result<Vec<TimeBucket>, ContractError> {
        Self::get_time_series(env, metric, start_time, end_time, window.seconds())
    }

    /// Returns the overall cumulative summary for a global metric.
    pub fn get_metric_summary(env: Env, metric: Symbol) -> Result<MetricSummary, ContractError> {
        let key = DataKey::MetricSummary(metric);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::SnapshotNotFound)
    }

    /// Returns the overall cumulative summary for a market metric.
    pub fn get_market_metric_summary(
        env: Env,
        market_id: Symbol,
        metric: Symbol,
    ) -> Result<MetricSummary, ContractError> {
        let key = DataKey::MarketMetricSummary(market_id, metric);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::MarketNotFound)
    }

    /// Returns raw recorded data points within `[start_time, end_time]` up to `limit`.
    pub fn get_data_points(
        env: Env,
        metric: Symbol,
        start_time: u64,
        end_time: u64,
        limit: u32,
    ) -> Result<Vec<DataPoint>, ContractError> {
        Self::assert_not_paused(&env)?;
        if end_time < start_time {
            return Err(ContractError::InvalidTimeRange);
        }

        let key = DataKey::MetricPoints(metric);
        let points: Vec<DataPoint> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env));

        let mut result = Vec::new(&env);
        let mut count = 0u32;
        for i in 0..points.len() {
            if count >= limit && limit > 0 {
                break;
            }
            if let Some(pt) = points.get(i) {
                if pt.timestamp >= start_time && pt.timestamp <= end_time {
                    result.push_back(pt);
                    count += 1;
                }
            }
        }

        Ok(result)
    }

    // =========================================================================
    // Legacy / Statistical Compatibility Functions
    // =========================================================================

    pub fn record_event(_env: Env, event_type: u32, value: u64) -> u64 {
        event_type as u64 + value
    }

    pub fn aggregate(_env: Env, event_types: Vec<u32>) -> u64 {
        event_types.iter().map(|x| x as u64).sum()
    }

    pub fn percentile(_env: Env, values: Vec<u64>, pct: u32) -> u64 {
        let len = values.len();
        if len == 0 {
            return 0;
        }
        let mut sorted = values.clone();
        for i in 1..len {
            let key = sorted.get(i).unwrap();
            let mut j = i;
            while j > 0 && sorted.get(j - 1).unwrap() > key {
                sorted.set(j, sorted.get(j - 1).unwrap());
                j -= 1;
            }
            sorted.set(j, key);
        }
        let idx = (pct * (len - 1)) / 100;
        sorted.get(idx).unwrap_or(0)
    }

    // =========================================================================
    // Internal Helper Functions
    // =========================================================================

    fn assert_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;
        if caller != &admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
        if Self::is_paused(env.clone()) {
            return Err(ContractError::AnalyticsPaused);
        }
        Ok(())
    }

    fn validate_time_query(
        start_time: u64,
        end_time: u64,
        window_size: u64,
    ) -> Result<(), ContractError> {
        if end_time < start_time {
            return Err(ContractError::InvalidTimeRange);
        }
        if window_size < storage::MIN_WINDOW_SECONDS || window_size > storage::MAX_WINDOW_SECONDS {
            return Err(ContractError::UnsupportedWindow);
        }
        let span = end_time - start_time;
        let estimated_buckets = span / window_size + 1;
        if estimated_buckets > storage::MAX_BUCKETS_PER_QUERY {
            return Err(ContractError::InvalidTimeRange);
        }
        Ok(())
    }

    /// Atomically checks capacity and writes a new data point.
    ///
    /// # Ordering guarantee
    ///
    /// The `StoreFull` check reads the persisted `total_count` from the
    /// `MetricSummary` and returns an error **before** any storage write
    /// occurs.  This prevents the store from ever exceeding `max_capacity`:
    /// the N-th call fills the stream to exactly `max_capacity`, and the
    /// (N+1)-th call returns `StoreFull` with the stream state unchanged.
    fn store_point(
        env: &Env,
        points_key: &DataKey,
        summary_key: &DataKey,
        value: u64,
        timestamp: u64,
    ) -> Result<(), ContractError> {
        let max_capacity: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MaxCapacity)
            .unwrap_or(storage::DEFAULT_MAX_CAPACITY);

        // Load the current summary and derive the committed point count.
        // This read happens before any write, so the check is an accurate
        // pre-condition: if it passes, we know there is room for one more point.
        let summary = env
            .storage()
            .persistent()
            .get::<DataKey, MetricSummary>(summary_key);
        let current_count = summary.as_ref().map(|s| s.total_count).unwrap_or(0);

        // Capacity guard: must come before any storage mutation.
        if current_count >= max_capacity {
            return Err(ContractError::StoreFull);
        }

        // All checks passed — perform the writes.
        let mut points: Vec<DataPoint> = env
            .storage()
            .persistent()
            .get(points_key)
            .unwrap_or_else(|| Vec::new(env));

        let next_summary = if let Some(mut s) = summary {
            s.total_count = s
                .total_count
                .checked_add(1)
                .ok_or(ContractError::Overflow)?;
            s.total_sum = s
                .total_sum
                .checked_add(value)
                .ok_or(ContractError::Overflow)?;
            if value < s.min_value {
                s.min_value = value;
            }
            if value > s.max_value {
                s.max_value = value;
            }
            s.latest_value = value;
            s.latest_timestamp = timestamp;
            s
        } else {
            MetricSummary {
                total_count: 1,
                total_sum: value,
                min_value: value,
                max_value: value,
                latest_value: value,
                first_timestamp: timestamp,
                latest_timestamp: timestamp,
            }
        };

        points.push_back(DataPoint { timestamp, value });
        env.storage().persistent().set(points_key, &points);
        env.storage()
            .persistent()
            .set(summary_key, &next_summary);
        Ok(())
    }

    fn compute_time_buckets(
        env: &Env,
        points: &Vec<DataPoint>,
        start_time: u64,
        end_time: u64,
        window_size: u64,
    ) -> Result<Vec<TimeBucket>, ContractError> {
        let mut buckets = Vec::new(env);
        if points.is_empty() {
            return Ok(buckets);
        }

        // Streaming single-pass aggregation into buckets
        let mut current_bucket_opt: Option<(u64, u32, u64, u64, u64, u64, u64)> = None;

        for i in 0..points.len() {
            if let Some(pt) = points.get(i) {
                if pt.timestamp < start_time || pt.timestamp > end_time {
                    continue;
                }

                let pt_bucket_start = (pt.timestamp / window_size) * window_size;

                match current_bucket_opt {
                    Some((
                        b_start,
                        ref mut b_count,
                        ref mut b_sum,
                        ref mut b_min,
                        ref mut b_max,
                        _b_open,
                        ref mut b_close,
                    )) => {
                        if b_start == pt_bucket_start {
                            *b_count += 1;
                            *b_sum = b_sum.checked_add(pt.value).ok_or(ContractError::Overflow)?;
                            if pt.value < *b_min {
                                *b_min = pt.value;
                            }
                            if pt.value > *b_max {
                                *b_max = pt.value;
                            }
                            *b_close = pt.value;
                        } else {
                            let avg = *b_sum / (*b_count as u64);
                            buckets.push_back(TimeBucket {
                                timestamp: b_start,
                                count: *b_count,
                                sum: *b_sum,
                                min: *b_min,
                                max: *b_max,
                                open: _b_open,
                                close: *b_close,
                                avg,
                            });
                            current_bucket_opt = Some((
                                pt_bucket_start,
                                1,
                                pt.value,
                                pt.value,
                                pt.value,
                                pt.value,
                                pt.value,
                            ));
                        }
                    }
                    None => {
                        current_bucket_opt = Some((
                            pt_bucket_start,
                            1,
                            pt.value,
                            pt.value,
                            pt.value,
                            pt.value,
                            pt.value,
                        ));
                    }
                }
            }
        }

        if let Some((b_start, b_count, b_sum, b_min, b_max, b_open, b_close)) = current_bucket_opt {
            let avg = b_sum / (b_count as u64);
            buckets.push_back(TimeBucket {
                timestamp: b_start,
                count: b_count,
                sum: b_sum,
                min: b_min,
                max: b_max,
                open: b_open,
                close: b_close,
                avg,
            });
        }

        Ok(buckets)
    }
}
