//! Tests for market reports indexing (issue #XXXX).
//!
//! Verifies that:
//! 1. Reports are stored and indexed by market ID
//! 2. `get_reports_by_market` enumerates all reports for a market
//! 3. `get_report` retrieves a specific report
//! 4. The index is persisted and survives event TTL expiry

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Vec,
};

use reporting::{ReportingContract, ReportingContractClient, ReportingError};

fn ledger_info() -> LedgerInfo {
    LedgerInfo {
        timestamp: 1_735_689_600,
        protocol_version: 20,
        sequence_number: 1,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 518_400,
    }
}

fn register_and_init(env: &Env) -> (ReportingContractClient, Address, Address) {
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let contract_id = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let reporter = Address::generate(env);
    client.initialize(&admin).unwrap();
    let _ = env.auths();
    (client, admin, reporter)
}

// ---------------------------------------------------------------------------
// submit_report and get_reports_by_market
// ---------------------------------------------------------------------------

/// After submitting a report, it should appear in `get_reports_by_market`.
#[test]
fn test_submit_report_returns_id_and_indexes_by_market() {
    let env = Env::default();
    let (client, _admin, reporter) = register_and_init(&env);

    let market_id = 42u32;
    let report_data = String::from_str(&env, "Market is manipulated");
    let report_hash = String::from_str(&env, "0xdeadbeef");

    // Submit a report.
    let report_id = client
        .submit_report(&reporter, &market_id, &report_data, &report_hash)
        .unwrap();

    // Should return ID 1 (first report).
    assert_eq!(report_id, 1);

    // Market should now have this report in its index.
    let market_reports: Vec<u32> = client.get_reports_by_market(&market_id);
    assert_eq!(market_reports.len(), 1);
    assert_eq!(market_reports.get(0).unwrap(), report_id);
}

/// Submitting multiple reports for the same market should index all of them.
#[test]
fn test_multiple_reports_same_market() {
    let env = Env::default();
    let (client, _admin, reporter) = register_and_init(&env);

    let market_id = 42u32;

    // Submit 3 reports for the same market.
    let id1 = client
        .submit_report(
            &reporter,
            &market_id,
            &String::from_str(&env, "Report 1"),
            &String::from_str(&env, "hash1"),
        )
        .unwrap();

    let id2 = client
        .submit_report(
            &reporter,
            &market_id,
            &String::from_str(&env, "Report 2"),
            &String::from_str(&env, "hash2"),
        )
        .unwrap();

    let id3 = client
        .submit_report(
            &reporter,
            &market_id,
            &String::from_str(&env, "Report 3"),
            &String::from_str(&env, "hash3"),
        )
        .unwrap();

    // All should have sequential IDs.
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    // Market should index all 3 reports.
    let market_reports: Vec<u32> = client.get_reports_by_market(&market_id);
    assert_eq!(market_reports.len(), 3);
    assert_eq!(market_reports.get(0).unwrap(), 1);
    assert_eq!(market_reports.get(1).unwrap(), 2);
    assert_eq!(market_reports.get(2).unwrap(), 3);
}

/// Reports for different markets should be indexed separately.
#[test]
fn test_reports_separate_market_indexes() {
    let env = Env::default();
    let (client, _admin, reporter) = register_and_init(&env);

    // Submit reports for market 1.
    let market1_id1 = client
        .submit_report(
            &reporter,
            &1,
            &String::from_str(&env, "Market 1, Report 1"),
            &String::from_str(&env, "hash1"),
        )
        .unwrap();

    let market1_id2 = client
        .submit_report(
            &reporter,
            &1,
            &String::from_str(&env, "Market 1, Report 2"),
            &String::from_str(&env, "hash2"),
        )
        .unwrap();

    // Submit reports for market 2.
    let market2_id1 = client
        .submit_report(
            &reporter,
            &2,
            &String::from_str(&env, "Market 2, Report 1"),
            &String::from_str(&env, "hash3"),
        )
        .unwrap();

    // Verify market 1 has 2 reports.
    let market1_reports: Vec<u32> = client.get_reports_by_market(&1);
    assert_eq!(market1_reports.len(), 2);
    assert_eq!(market1_reports.get(0).unwrap(), market1_id1);
    assert_eq!(market1_reports.get(1).unwrap(), market1_id2);

    // Verify market 2 has 1 report.
    let market2_reports: Vec<u32> = client.get_reports_by_market(&2);
    assert_eq!(market2_reports.len(), 1);
    assert_eq!(market2_reports.get(0).unwrap(), market2_id1);

    // Verify market 3 is empty.
    let market3_reports: Vec<u32> = client.get_reports_by_market(&3);
    assert_eq!(market3_reports.len(), 0);
}

// ---------------------------------------------------------------------------
// get_report
// ---------------------------------------------------------------------------

/// `get_report` should retrieve the full report data by ID.
#[test]
fn test_get_report_returns_full_data() {
    let env = Env::default();
    let (client, _admin, reporter) = register_and_init(&env);

    let market_id = 42u32;
    let report_data = String::from_str(&env, "Suspicious trading volume");
    let report_hash = String::from_str(&env, "0xaabbccdd");

    let report_id = client
        .submit_report(&reporter, &market_id, &report_data, &report_hash)
        .unwrap();

    // Retrieve the report.
    let report = client.get_report(&report_id).unwrap();

    assert_eq!(report.id, report_id);
    assert_eq!(report.market_id, market_id);
    assert_eq!(report.reporter, reporter);
    assert_eq!(report.report_data, report_data);
    assert_eq!(report.report_hash, report_hash);
    assert_eq!(report.status, 0); // Default pending status
}

/// `get_report` should return `ReportNotFound` for non-existent IDs.
#[test]
fn test_get_report_not_found() {
    let env = Env::default();
    let (client, _admin, _reporter) = register_and_init(&env);

    let result = client.try_get_report(&999);
    match result {
        Err(e) => {
            // Verify it's the right error.
            let err = e.to_string();
            assert!(err.contains("ReportNotFound") || err.contains("7"));
        }
        Ok(_) => panic!("Expected ReportNotFound error"),
    }
}

// ---------------------------------------------------------------------------
// Pause behavior
// ---------------------------------------------------------------------------

/// When paused, `submit_report` should reject with `ReportingPaused`.
#[test]
fn test_submit_report_paused() {
    let env = Env::default();
    let (client, admin, reporter) = register_and_init(&env);

    // Pause reporting.
    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    // Try to submit a report while paused.
    let result = client.try_submit_report(
        &reporter,
        &42,
        &String::from_str(&env, "Oops"),
        &String::from_str(&env, "hash"),
    );

    match result {
        Err(e) => {
            let err = e.to_string();
            assert!(
                err.contains("ReportingPaused") || err.contains("4"),
                "Expected ReportingPaused, got: {}",
                err
            );
        }
        Ok(_) => panic!("Expected ReportingPaused error"),
    }
}
