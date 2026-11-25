//! Per-entrypoint Soroban CPU and memory regression snapshots.
//!
//! Each snapshot isolates one public call and compares its measured mock-host
//! budget delta with thresholds in `benches/thresholds.json`. CI permits at
//! most a 5% increase: `limit = threshold + ceil(threshold / 20)`. Setup work
//! is deliberately completed before the budget reset so unrelated fixture
//! costs cannot hide an entrypoint regression.

use admin::{AdminContract, AdminContractClient, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};
use std::collections::HashMap;

#[derive(Copy, Clone)]
struct Threshold {
    max_cpu_insns: u64,
    max_mem_bytes: u64,
}

fn load_thresholds() -> HashMap<String, Threshold> {
    const RAW: &str = include_str!("../benches/thresholds.json");
    let doc: serde_json::Value =
        serde_json::from_str(RAW).expect("benches/thresholds.json must be valid JSON");
    let entries = doc["thresholds"]
        .as_object()
        .expect("thresholds.json must have a 'thresholds' object");

    entries
        .iter()
        .map(|(name, value)| {
            let cpu = value["max_cpu_insns"]
                .as_u64()
                .unwrap_or_else(|| panic!("threshold '{name}' missing max_cpu_insns"));
            let memory = value["max_mem_bytes"]
                .as_u64()
                .unwrap_or_else(|| panic!("threshold '{name}' missing max_mem_bytes"));
            (
                name.clone(),
                Threshold {
                    max_cpu_insns: cpu,
                    max_mem_bytes: memory,
                },
            )
        })
        .collect()
}

fn threshold<'a>(thresholds: &'a HashMap<String, Threshold>, name: &str) -> &'a Threshold {
    thresholds
        .get(name)
        .unwrap_or_else(|| panic!("thresholds.json is missing required key '{name}'"))
}

fn measured_budget(env: &Env) -> (u64, u64) {
    let budget = env.cost_estimate().budget();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

fn reset_budget(env: &Env) {
    env.cost_estimate().budget().reset_unlimited();
}

fn five_percent_ceiling(threshold: u64) -> u64 {
    let rounded_five_percent = threshold.div_ceil(20);
    threshold
        .checked_add(rounded_five_percent)
        .expect("gas threshold must leave room for its 5% regression margin")
}

fn assert_budget(label: &str, measured: (u64, u64), threshold: &Threshold) {
    let cpu_limit = five_percent_ceiling(threshold.max_cpu_insns);
    let memory_limit = five_percent_ceiling(threshold.max_mem_bytes);

    println!(
        "{label}: cpu={}, memory={}, cpu_limit={cpu_limit}, memory_limit={memory_limit}",
        measured.0, measured.1
    );
    assert!(
        measured.0 <= cpu_limit,
        "{label}: CPU regression exceeds 5%: measured {}, threshold {}, limit {}",
        measured.0,
        threshold.max_cpu_insns,
        cpu_limit
    );
    assert!(
        measured.1 <= memory_limit,
        "{label}: memory regression exceeds 5%: measured {}, threshold {}, limit {}",
        measured.1,
        threshold.max_mem_bytes,
        memory_limit
    );
}

struct Fixture {
    env: Env,
    contract_id: Address,
    admin: Address,
}

impl Fixture {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AdminContract, ());
        let admin = Address::generate(&env);
        Self {
            env,
            contract_id,
            admin,
        }
    }

    fn client(&self) -> AdminContractClient<'_> {
        AdminContractClient::new(&self.env, &self.contract_id)
    }

    fn initialize(&self) {
        self.client().initialize(&self.admin);
    }
}

#[test]
fn snapshot_initialize() {
    let fixture = Fixture::new();
    let client = fixture.client();
    let thresholds = load_thresholds();

    reset_budget(&fixture.env);
    client.initialize(&fixture.admin);
    let measured = measured_budget(&fixture.env);

    assert_budget("initialize", measured, threshold(&thresholds, "initialize"));
}

#[test]
fn snapshot_admin() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    let thresholds = load_thresholds();

    reset_budget(&fixture.env);
    let stored_admin = client.admin();
    let measured = measured_budget(&fixture.env);

    assert_eq!(stored_admin, Ok(fixture.admin));
    assert_budget("admin", measured, threshold(&thresholds, "admin"));
}

#[test]
fn snapshot_set_admin_cooldown() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    let thresholds = load_thresholds();

    reset_budget(&fixture.env);
    client.set_admin_cooldown(&fixture.admin, &300);
    let measured = measured_budget(&fixture.env);

    assert_eq!(client.get_admin_cooldown(), 300);
    assert_budget(
        "set_admin_cooldown",
        measured,
        threshold(&thresholds, "set_admin_cooldown"),
    );
}

#[test]
fn snapshot_get_admin_cooldown() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    client.set_admin_cooldown(&fixture.admin, &600);
    let thresholds = load_thresholds();

    reset_budget(&fixture.env);
    let cooldown = client.get_admin_cooldown();
    let measured = measured_budget(&fixture.env);

    assert_eq!(cooldown, 600);
    assert_budget(
        "get_admin_cooldown",
        measured,
        threshold(&thresholds, "get_admin_cooldown"),
    );
}

#[test]
fn snapshot_check_admin_cooldown() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    client.set_admin_cooldown(&fixture.admin, &300);
    let thresholds = load_thresholds();

    let func_name = Symbol::new(&fixture.env, "test_action");

    reset_budget(&fixture.env);
    let result = client.check_admin_cooldown(&fixture.admin, &func_name);
    let measured = measured_budget(&fixture.env);

    assert!(result.is_ok());
    assert_budget(
        "check_admin_cooldown",
        measured,
        threshold(&thresholds, "check_admin_cooldown"),
    );
}

#[test]
fn rejects_duplicate_initialization() {
    let fixture = Fixture::new();
    fixture.initialize();

    assert_eq!(
        fixture.client().try_initialize(&fixture.admin),
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

#[test]
fn rejects_unauthorized_admin_call() {
    let fixture = Fixture::new();
    fixture.initialize();
    let stranger = Address::generate(&fixture.env);

    assert_eq!(
        fixture.client().try_set_admin_cooldown(&stranger, &100),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        fixture.client().try_disable_admin_cooldown(&stranger),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn rejects_cooldowns_below_minimum() {
    let fixture = Fixture::new();
    fixture.initialize();

    assert_eq!(
        fixture
            .client()
            .try_set_admin_cooldown(&fixture.admin, &0),
        Err(Ok(ContractError::InvalidCooldown))
    );
    assert_eq!(
        fixture
            .client()
            .try_set_admin_cooldown(&fixture.admin, &59),
        Err(Ok(ContractError::InvalidCooldown))
    );
}

#[test]
fn disable_admin_cooldown_is_explicit() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    let function_name = Symbol::new(&fixture.env, "test_action");

    fixture.env.ledger().with_mut(|ledger| {
        ledger.timestamp = 100;
    });
    client.set_admin_cooldown(&fixture.admin, &300);
    assert!(client
        .check_admin_cooldown(&fixture.admin, &function_name)
        .is_ok());

    client.disable_admin_cooldown(&fixture.admin);
    assert_eq!(client.get_admin_cooldown(), 0);
    assert!(client
        .check_admin_cooldown(&fixture.admin, &function_name)
        .is_ok());
}

#[test]
fn rejects_cooldown_when_active() {
    let fixture = Fixture::new();
    fixture.initialize();
    let client = fixture.client();
    client.set_admin_cooldown(&fixture.admin, &300);

    let func_name = Symbol::new(&fixture.env, "test_action");

    // First call should succeed
    assert!(client.check_admin_cooldown(&fixture.admin, &func_name).is_ok());

    // Immediate second call should fail due to cooldown
    assert_eq!(
        client.try_check_admin_cooldown(&fixture.admin, &func_name),
        Err(Ok(ContractError::AdminActionTimelocked))
    );
}

#[test]
fn regression_margin_is_exactly_five_percent_rounded_up() {
    assert_eq!(five_percent_ceiling(0), 0);
    assert_eq!(five_percent_ceiling(1), 2);
    assert_eq!(five_percent_ceiling(20), 21);
    assert_eq!(five_percent_ceiling(21), 23);
    assert_eq!(five_percent_ceiling(100), 105);
}

#[test]
fn thresholds_json_has_all_required_keys() {
    let thresholds = load_thresholds();
    for key in &[
        "initialize",
        "admin",
        "set_admin_cooldown",
        "get_admin_cooldown",
        "check_admin_cooldown",
    ] {
        assert!(
            thresholds.contains_key(*key),
            "thresholds.json is missing required key '{key}'"
        );
    }
}

#[test]
fn thresholds_json_values_are_positive() {
    let thresholds = load_thresholds();
    for (key, threshold) in &thresholds {
        assert!(
            threshold.max_cpu_insns > 0,
            "threshold '{key}': max_cpu_insns must be > 0"
        );
        assert!(
            threshold.max_mem_bytes > 0,
            "threshold '{key}': max_mem_bytes must be > 0"
        );
    }
}
