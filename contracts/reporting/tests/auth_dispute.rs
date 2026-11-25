#![cfg(test)]

mod common;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use reporting::{ReportingContract, ReportingContractClient, ReportingError};

use common::{dispute_reason, ledger_info, register_and_init};

#[test]
fn test_dispute_report_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let reporter = Address::generate(&env);

    let result = client.try_dispute_report(&reporter, &1u32, &dispute_reason(&env));
    assert!(result.is_err(), "dispute_report must require auth; succeeded without it");
}

#[test]
fn test_dispute_report_accepted_with_reporter_auth() {
    let env = Env::default();
    let (client, _admin) = register_and_init(&env);
    let reporter = Address::generate(&env);

    client
        .dispute_report(&reporter, &1u32, &dispute_reason(&env))
        .unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, reporter, "reporter must be the authorised address");
}

#[test]
fn test_dispute_report_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let reporter = Address::generate(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_dispute_report(&reporter, &1u32, &dispute_reason(&env));
    assert!(result.is_err(), "dispute_report should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}

#[test]
fn test_resolve_dispute_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_resolve_dispute(&admin, &1u32, &true);
    assert!(result.is_err(), "resolve_dispute must require auth; succeeded without it");
}

#[test]
fn test_resolve_dispute_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.resolve_dispute(&admin, &1u32, &true).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
}

#[test]
fn test_resolve_dispute_role_separation_reporter_cannot_resolve() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let reporter = Address::generate(&env);

    let result = client.try_resolve_dispute(&reporter, &1u32, &true);
    assert!(result.is_err(), "reporter should not be able to call resolve_dispute without auth");
}

#[test]
fn test_resolve_dispute_blocked_when_paused() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result = client.try_resolve_dispute(&admin, &1u32, &true);
    assert!(result.is_err(), "resolve_dispute should fail when paused");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::ReportingPaused),
        Err(_) => panic!("expected ReportingPaused, got host error"),
    }
}
