#![cfg(test)]

mod common;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use reporting::{ReportingContract, ReportingContractClient, ReportingError};

use common::{
    ledger_info, register_and_init, report_data, report_hash,
};

#[test]
fn test_submit_report_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let reporter = Address::generate(&env);

    let result = client.try_submit_report(
        &reporter,
        &1u32,
        &report_data(&env),
        &report_hash(&env),
    );
    assert!(result.is_err(), "submit_report must require auth; succeeded without it");
}

#[test]
fn test_submit_report_accepted_with_reporter_auth() {
    let env = Env::default();
    let (client, _admin) = register_and_init(&env);
    let reporter = Address::generate(&env);

    client
        .submit_report(&reporter, &1u32, &report_data(&env), &report_hash(&env))
        .unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1, "exactly one auth entry expected");
    assert_eq!(auths[0].0, reporter, "reporter must be the authorised address");
}

#[test]
fn test_submit_report_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let reporter = Address::generate(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_submit_report(
        &reporter,
        &1u32,
        &report_data(&env),
        &report_hash(&env),
    );
    assert!(result.is_err(), "submit_report should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}
