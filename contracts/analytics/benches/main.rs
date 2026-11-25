use analytics::{Analytics, AnalyticsClient};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

fn bench_record_event(c: &mut Criterion) {
    c.bench_function("record_event", |b| {
        b.iter(|| black_box(0u32 as u64 + black_box(100u64)))
    });
}

fn bench_aggregate(c: &mut Criterion) {
    c.bench_function("aggregate_1000", |b| {
        b.iter(|| black_box((0u64..1000).sum::<u64>()))
    });
}

fn bench_percentile(c: &mut Criterion) {
    c.bench_function("percentile_p95_1000", |b| {
        b.iter(|| {
            let mut v: std::vec::Vec<u64> = (0u64..1000).collect();
            v.sort();
            black_box(v.get(949).copied().unwrap_or(0))
        })
    });
}

fn bench_contract_record_metric(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let _ = client.initialize(&admin);
    let metric = symbol_short!("volume");

    let mut counter = 100u64;
    c.bench_function("contract_record_metric", |b| {
        b.iter(|| {
            counter += 1;
            black_box(client.record_metric_at(&metric, &counter, &counter))
        })
    });
}

fn bench_contract_time_series_query(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let _ = client.initialize(&admin);
    let metric = symbol_short!("tx_cnt");

    // Populate with 200 data points across time
    for t in 0..200u64 {
        let _ = client.record_metric_at(&metric, &(t * 10), &(t * 60));
    }

    c.bench_function("contract_get_time_series_200_pts", |b| {
        b.iter(|| {
            black_box(client.get_time_series(&metric, &0u64, &12000u64, &300u64))
        })
    });
}

fn bench_contract_summary_query(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(Analytics, ());
    let client = AnalyticsClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let _ = client.initialize(&admin);
    let metric = symbol_short!("sum_m");

    for t in 0..100u64 {
        let _ = client.record_metric_at(&metric, &(t * 5), &(t * 10));
    }

    c.bench_function("contract_get_metric_summary", |b| {
        b.iter(|| black_box(client.get_metric_summary(&metric)))
    });
}

criterion_group!(
    benches,
    bench_record_event,
    bench_aggregate,
    bench_percentile,
    bench_contract_record_metric,
    bench_contract_time_series_query,
    bench_contract_summary_query
);
criterion_main!(benches);
