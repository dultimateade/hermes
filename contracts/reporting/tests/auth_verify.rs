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
fn test_verify_report_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_verify_report(&admin, &1u32, &true);
    assert!(result.is_err(), "verify_report must require auth; succeeded without it");
}

#[test]
fn test_verify_report_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.verify_report(&admin, &1u32, &true).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
}

#[test]
fn test_verify_report_role_separation_reporter_cannot_verify() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let reporter = Address::generate(&env);

    let result = client.try_verify_report(&reporter, &1u32, &true);
    assert!(result.is_err(), "reporter should not be able to call verify_report without auth");
}

#[test]
fn test_verify_report_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_verify_report(&admin, &1u32, &true);
    assert!(result.is_err(), "verify_report should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}

#[test]
fn test_update_report_status_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_update_report_status(&admin, &1u32, &2u32);
    assert!(result.is_err(), "update_report_status must require auth; succeeded without it");
}

#[test]
fn test_update_report_status_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.update_report_status(&admin, &1u32, &2u32).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
}

#[test]
fn test_update_report_status_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_update_report_status(&admin, &1u32, &2u32);
    assert!(result.is_err(), "update_report_status should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}

#[test]
fn test_delete_report_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_delete_report(&admin, &1u32);
    assert!(result.is_err(), "delete_report must require auth; succeeded without it");
}

#[test]
fn test_delete_report_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let reporter = Address::generate(&env);
    client
        .submit_report(&reporter, &1u32, &report_data(&env), &report_hash(&env))
        .unwrap();
    let _ = env.auths();

    client.delete_report(&admin, &1u32).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
}

#[test]
fn test_delete_report_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_delete_report(&admin, &1u32);
    assert!(result.is_err(), "delete_report should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}
