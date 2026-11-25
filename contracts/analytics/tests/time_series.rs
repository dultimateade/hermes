#![cfg(test)]

use analytics::{
    Analytics, AnalyticsClient, ContractError, MetricSummary, TimeBucket, TimeWindow,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    vec, Address, Env,
};

fn setup_env<'a>() -> (Env, AnalyticsClient<'a>, Address) {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_initialize_and_admin_management() {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    env.mock_all_auths();

    // First initialization succeeds
    assert_eq!(client.try_initialize(&admin1), Ok(Ok(())));
    assert_eq!(client.get_admin(), admin1);

    // Second initialization fails with AlreadyInitialized
    assert_eq!(
        client.try_initialize(&admin1),
        Err(Ok(ContractError::AlreadyInitialized))
    );

    // Set new admin
    assert_eq!(client.try_set_admin(&admin1, &admin2), Ok(Ok(())));
    assert_eq!(client.get_admin(), admin2);

    // Old admin cannot perform admin actions
    assert_eq!(
        client.try_set_admin(&admin1, &admin1),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn test_pause_and_unpause() {
    let (_env, client, admin) = setup_env();
    let metric = symbol_short!("volume");

    assert_eq!(client.is_paused(), false);

    // Record metric works
    assert_eq!(client.record_metric_at(&metric, &100, &1000), 100);

    // Pause contract
    client.set_paused(&admin, &true);
    assert_eq!(client.is_paused(), true);

    // Recording and querying fail when paused
    assert_eq!(
        client.try_record_metric_at(&metric, &200, &2000),
        Err(Ok(ContractError::AnalyticsPaused))
    );
    assert_eq!(
        client.try_get_time_series(&metric, &0, &5000, &60),
        Err(Ok(ContractError::AnalyticsPaused))
    );

    // Unpause
    client.set_paused(&admin, &false);
    assert_eq!(client.is_paused(), false);
    assert_eq!(client.record_metric_at(&metric, &200, &2000), 200);
}

#[test]
fn test_record_metric_at_rejects_timestamp_outside_ledger_drift() {
    let (env, client, _admin) = setup_env();
    env.ledger().set_timestamp(100_000);
    let metric = symbol_short!("future");
    let timestamp = env.ledger().timestamp() + 86_401;
    let stale_timestamp = env.ledger().timestamp() - 86_401;

    assert_eq!(
        client.try_record_metric_at(&metric, &timestamp, &100),
        Err(Ok(ContractError::ValueOutOfRange))
    );
    assert_eq!(
        client.try_record_metric_at(&metric, &stale_timestamp, &100),
        Err(Ok(ContractError::ValueOutOfRange))
    );
}

#[test]
fn test_time_bucketed_aggregation() {
    let (env, client, _admin) = setup_env();
    let metric = symbol_short!("price");

    // Record points across multiple hourly windows:
    // Window 1: [0, 3600)
    // Points at t=100 (val=10), t=500 (val=25), t=3500 (val=15)
    // Window 2: [3600, 7200)
    // Points at t=3600 (val=50), t=4000 (val=70), t=7199 (val=30)
    // Window 3: [7200, 10800)
    // No points
    // Window 4: [10800, 14400)
    // Point at t=11000 (val=100)

    client.record_metric_at(&metric, &10, &100);
    client.record_metric_at(&metric, &25, &500);
    client.record_metric_at(&metric, &15, &3500);

    client.record_metric_at(&metric, &50, &3600);
    client.record_metric_at(&metric, &70, &4000);
    client.record_metric_at(&metric, &30, &7199);

    client.record_metric_at(&metric, &100, &11000);

    // Query 1-hour windows covering range 0 to 12000
    let window_1h = 3600u64;
    let series = client.get_time_series(&metric, &0, &12000, &window_1h);

    assert_eq!(series.len(), 3);

    // Verify Bucket 1 (t=0)
    let b1 = series.get(0).unwrap();
    assert_eq!(b1.timestamp, 0);
    assert_eq!(b1.count, 3);
    assert_eq!(b1.sum, 50); // 10 + 25 + 15
    assert_eq!(b1.min, 10);
    assert_eq!(b1.max, 25);
    assert_eq!(b1.open, 10);
    assert_eq!(b1.close, 15);
    assert_eq!(b1.avg, 16); // 50 / 3 = 16

    // Verify Bucket 2 (t=3600)
    let b2 = series.get(1).unwrap();
    assert_eq!(b2.timestamp, 3600);
    assert_eq!(b2.count, 3);
    assert_eq!(b2.sum, 150); // 50 + 70 + 30
    assert_eq!(b2.min, 30);
    assert_eq!(b2.max, 70);
    assert_eq!(b2.open, 50);
    assert_eq!(b2.close, 30);
    assert_eq!(b2.avg, 50); // 150 / 3 = 50

    // Verify Bucket 3 (t=10800)
    let b3 = series.get(2).unwrap();
    assert_eq!(b3.timestamp, 10800);
    assert_eq!(b3.count, 1);
    assert_eq!(b3.sum, 100);
    assert_eq!(b3.min, 100);
    assert_eq!(b3.max, 100);
    assert_eq!(b3.open, 100);
    assert_eq!(b3.close, 100);
    assert_eq!(b3.avg, 100);

    // Query using enum TimeWindow::OneHour
    let enum_series = client.get_time_buckets(&metric, &0, &12000, &TimeWindow::OneHour);
    assert_eq!(enum_series, series);

    // Query sub-range [3000, 5000]
    let sub_series = client.get_time_series(&metric, &3000, &5000, &window_1h);
    assert_eq!(sub_series.len(), 2);
    assert_eq!(sub_series.get(0).unwrap().count, 1); // point at 3500
    assert_eq!(sub_series.get(1).unwrap().count, 2); // points at 3600, 4000
}

#[test]
fn test_market_specific_time_series() {
    let (_env, client, _admin) = setup_env();
    let market_a = symbol_short!("mkt_a");
    let market_b = symbol_short!("mkt_b");
    let metric = symbol_short!("bets");

    client.record_market_metric_at(&market_a, &metric, &500, &1000);
    client.record_market_metric_at(&market_a, &metric, &1500, &1200);

    client.record_market_metric_at(&market_b, &metric, &300, &1000);

    let series_a = client.get_market_time_series(&market_a, &metric, &0, &2000, &60);
    assert_eq!(series_a.len(), 2);

    let summary_a = client.get_market_metric_summary(&market_a, &metric);
    assert_eq!(summary_a.total_count, 2);
    assert_eq!(summary_a.total_sum, 2000);
    assert_eq!(summary_a.min_value, 500);
    assert_eq!(summary_a.max_value, 1500);
    assert_eq!(summary_a.latest_value, 1500);

    let summary_b = client.get_market_metric_summary(&market_b, &metric);
    assert_eq!(summary_b.total_count, 1);
    assert_eq!(summary_b.total_sum, 300);

    // Non-existent market returns MarketNotFound
    let non_existent = symbol_short!("no_mkt");
    assert_eq!(
        client.try_get_market_metric_summary(&non_existent, &metric),
        Err(Ok(ContractError::MarketNotFound))
    );
}

#[test]
fn test_query_validation_and_errors() {
    let (_env, client, _admin) = setup_env();
    let metric = symbol_short!("trades");

    // Invalid time range (end < start)
    assert_eq!(
        client.try_get_time_series(&metric, &5000, &1000, &60),
        Err(Ok(ContractError::InvalidTimeRange))
    );

    // Unsupported window (0 seconds)
    assert_eq!(
        client.try_get_time_series(&metric, &1000, &5000, &0),
        Err(Ok(ContractError::UnsupportedWindow))
    );

    // Missing summary returns SnapshotNotFound
    let unknown_metric = symbol_short!("unknown");
    assert_eq!(
        client.try_get_metric_summary(&unknown_metric),
        Err(Ok(ContractError::SnapshotNotFound))
    );
}

#[test]
fn test_capacity_limits() {
    let (_env, client, admin) = setup_env();
    let metric = symbol_short!("ticks");

    // Set low capacity of 3
    client.set_capacity(&admin, &3);

    assert_eq!(client.record_metric_at(&metric, &10, &100), 10);
    assert_eq!(client.record_metric_at(&metric, &20, &200), 20);
    assert_eq!(client.record_metric_at(&metric, &30, &300), 30);

    // 4th point exceeds capacity
    assert_eq!(
        client.try_record_metric_at(&metric, &40, &400),
        Err(Ok(ContractError::StoreFull))
    );
    assert_eq!(client.get_metric_summary(&metric).total_count, 3);
}

/// The `StoreFull` guard fires **before** any storage write.
///
/// Specifically, when the store is at exactly `max_capacity`:
/// - The rejected call returns `StoreFull`.
/// - `total_count` in the summary remains at `max_capacity` (not `max_capacity + 1`).
/// - The raw data-point list also remains at `max_capacity` entries.
/// - A subsequent call with a *different* timestamp still returns `StoreFull`,
///   confirming the store is not in a partially-written inconsistent state.
#[test]
fn test_store_full_check_runs_before_write() {
    let (env, client, admin) = setup_env();
    let metric = symbol_short!("snap");

    // Fill to capacity = 2 exactly.
    client.set_capacity(&admin, &2);
    assert_eq!(client.record_metric_at(&metric, &10, &1000), 10);
    assert_eq!(client.record_metric_at(&metric, &20, &2000), 20);

    // Store is now at capacity; summary must reflect exactly 2 points.
    let summary_before = client.get_metric_summary(&metric);
    assert_eq!(summary_before.total_count, 2);

    // The 3rd call must return StoreFull without writing anything.
    assert_eq!(
        client.try_record_metric_at(&metric, &30, &3000),
        Err(Ok(ContractError::StoreFull))
    );

    // summary.total_count must still be 2 — no partial write occurred.
    let summary_after = client.get_metric_summary(&metric);
    assert_eq!(
        summary_after.total_count, 2,
        "StoreFull must not increment total_count"
    );
    assert_eq!(
        summary_after.total_sum, summary_before.total_sum,
        "StoreFull must not modify total_sum"
    );
    assert_eq!(
        summary_after.latest_value, summary_before.latest_value,
        "StoreFull must not modify latest_value"
    );

    // The raw points list must also still have exactly 2 entries.
    let pts = client.get_data_points(&metric, &0, &u64::MAX, &0);
    assert_eq!(
        pts.len(),
        2,
        "StoreFull must not append a data point to the points list"
    );

    // Another rejected call with a different value must leave the state unchanged.
    assert_eq!(
        client.try_record_metric_at(&metric, &99, &4000),
        Err(Ok(ContractError::StoreFull))
    );
    let summary_still = client.get_metric_summary(&metric);
    assert_eq!(summary_still.total_count, 2);
}

#[test]
fn test_legacy_methods() {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);

    assert_eq!(client.record_event(&5, &10), 15);
    assert_eq!(client.aggregate(&vec![&env, 10, 20, 30]), 60);
    assert_eq!(client.percentile(&vec![&env, 10, 20, 30, 40, 50], &50), 30);
    assert_eq!(client.percentile(&vec![&env, 10, 20, 30, 40, 50], &95), 40);
    assert_eq!(client.percentile(&vec![&env], &50), 0);
}
