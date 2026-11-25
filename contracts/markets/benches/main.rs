use criterion::{black_box, criterion_group, criterion_main, Criterion};
use markets::{MarketsContract, MarketsContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

// ---------------------------------------------------------------------------
// Micro-benchmarks (arithmetic proxies for on-chain operations)
// ---------------------------------------------------------------------------

fn bench_create_market(c: &mut Criterion) {
    // Proxy: models the counter increment + struct initialisation overhead
    c.bench_function("create_market", |b| {
        b.iter(|| {
            let counter: u32 = black_box(0u32);
            let market_id = black_box(counter.wrapping_add(1));
            black_box(market_id as u64 + black_box(1735689600u64))
        })
    });
}

fn bench_place_bet(c: &mut Criterion) {
    // Proxy: models outcome-index validation + pool accumulation arithmetic
    c.bench_function("place_bet", |b| {
        b.iter(|| {
            let amount: i128 = black_box(1000i128);
            let total_pool: i128 = black_box(5000i128);
            let new_total = black_box(total_pool.wrapping_add(amount));
            let outcome_pool: i128 = black_box(2000i128);
            let new_outcome_pool = black_box(outcome_pool.wrapping_add(amount));
            black_box((new_total, new_outcome_pool))
        })
    });
}

fn bench_resolve_market(c: &mut Criterion) {
    // Proxy: models winning-outcome validation + state flag update
    c.bench_function("resolve_market", |b| {
        b.iter(|| {
            let winning_outcome: u32 = black_box(1u32);
            let num_outcomes: u32 = black_box(2u32);
            let valid = black_box(winning_outcome < num_outcomes);
            black_box((valid, winning_outcome))
        })
    });
}

fn bench_claim_winnings(c: &mut Criterion) {
    // Proxy: models payout calculation (winner-takes-all path)
    c.bench_function("claim_winnings", |b| {
        b.iter(|| {
            let bet_amount: i128 = black_box(1000i128);
            let total_pool: i128 = black_box(5000i128);
            let win_pool: i128 = black_box(2500i128);
            let payout = black_box((bet_amount * total_pool) / win_pool);
            black_box(payout)
        })
    });
}

// ---------------------------------------------------------------------------
// Contract-level benchmarks (full Soroban environment invocations)
// ---------------------------------------------------------------------------

fn bench_contract_create_market(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Will ETH exceed $10k by 2027?");
    let description = String::from_str(&env, "Ethereum price prediction market");
    let resolution_source = String::from_str(&env, "CoinGecko");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
        ],
    );
    let end_time: u64 = 1_800_000_000;

    c.bench_function("contract_create_market", |b| {
        b.iter(|| {
            black_box(client.create_market(
                &creator,
                &question,
                &description,
                &end_time,
                &resolution_source,
                &outcome_tags,
                &black_box(1i128),
            ))
        })
    });
}

fn bench_contract_place_bet(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Will BTC reach $200k?");
    let description = String::from_str(&env, "Bitcoin prediction market");
    let resolution_source = String::from_str(&env, "CMC");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
        ],
    );
    let end_time: u64 = 1_800_000_000;

    let market_id = client.create_market(
        &creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );

    c.bench_function("contract_place_bet", |b| {
        b.iter(|| {
            black_box(client.place_bet(&user, &market_id, &0u32, &black_box(500i128)))
        })
    });
}

fn bench_contract_set_market_payout_curve(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Will DOT reach $100?");
    let description = String::from_str(&env, "Polkadot prediction market");
    let resolution_source = String::from_str(&env, "CoinGecko");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
        ],
    );
    let end_time: u64 = 1_800_000_000;

    let market_id = client.create_market(
        &creator,
        &question,
        &description,
        &end_time,
        &resolution_source,
        &outcome_tags,
        &1i128,
    );

    c.bench_function("contract_set_market_payout_curve", |b| {
        b.iter(|| {
            black_box(client.set_market_payout_curve(
                &creator,
                &market_id,
                &black_box(1u32),
            ))
        })
    });
}

fn bench_contract_propose_pause(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    c.bench_function("contract_propose_pause", |b| {
        b.iter(|| black_box(client.propose_pause(&admin, &black_box(true))))
    });
}

fn bench_contract_approve_pause(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    // A pause proposal must exist before it can be approved.
    client.propose_pause(&admin, &true);

    c.bench_function("contract_approve_pause", |b| {
        b.iter(|| black_box(client.approve_pause(&admin)))
    });
}

fn bench_contract_execute_pause(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    env.mock_all_auths();

    // A pause proposal must exist before it can be executed.
    client.propose_pause(&admin, &true);

    c.bench_function("contract_execute_pause", |b| {
        b.iter(|| black_box(client.execute_pause(&admin)))
    });
}

fn bench_contract_resolve_market(c: &mut Criterion) {
    let env = Env::default();
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    env.mock_all_auths();

    let question = String::from_str(&env, "Will SOL hit $1k?");
    let description = String::from_str(&env, "Solana prediction market");
    let resolution_source = String::from_str(&env, "CoinGecko");
    let outcome_tags = Vec::from_array(
        &env,
        [
            String::from_str(&env, "Yes"),
            String::from_str(&env, "No"),
        ],
    );
    let end_time: u64 = 1_800_000_000;

    // Each bench iteration creates a fresh market so resolve_market can run.
    c.bench_function("contract_resolve_market", |b| {
        b.iter(|| {
            let market_id = client.create_market(
                &creator,
                &question,
                &description,
                &end_time,
                &resolution_source,
                &outcome_tags,
                &1i128,
            );
            black_box(client.resolve_market(&creator, &market_id, &0u32))
        })
    });
}

criterion_group!(
    benches,
    bench_create_market,
    bench_place_bet,
    bench_resolve_market,
    bench_claim_winnings,
    bench_contract_create_market,
    bench_contract_place_bet,
    bench_contract_set_market_payout_curve,
    bench_contract_propose_pause,
    bench_contract_approve_pause,
    bench_contract_execute_pause,
    bench_contract_resolve_market,
);
criterion_main!(benches);
