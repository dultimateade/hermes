#![cfg(test)]

mod common;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use reporting::{ReportingContract, ReportingContractClient, ReportingError};

use common::{
    ledger_info, register, register_and_init, report_data, report_hash,
};

#[test]
fn test_initialize_rejected_without_auth() {
    let env = Env::default();
    let (client, admin) = register(&env);

    let result = client.try_initialize(&admin);
    assert!(result.is_err(), "initialize must require auth; succeeded without it");
}

#[test]
fn test_initialize_accepted_with_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = register(&env);

    client.initialize(&admin).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1, "exactly one auth entry expected");
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
}

#[test]
fn test_initialize_returns_already_initialized_on_double_init() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    let result = client.try_initialize(&admin);
    assert!(result.is_err(), "double-init must fail");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::AlreadyInitialized),
        Err(_) => panic!("expected contract error, got host error"),
    }
}

#[test]
fn test_pause_reporting_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_pause_reporting(&admin);
    assert!(result.is_err(), "pause_reporting must require auth; succeeded without it");
}

#[test]
fn test_pause_reporting_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
    assert!(client.is_reporting_paused(), "contract should be paused");
}

#[test]
fn test_pause_reporting_non_admin_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let non_admin = Address::generate(&env);

    let result = client.try_pause_reporting(&non_admin);
    assert!(result.is_err(), "non-admin must not be able to pause without auth");
}

#[test]
fn test_unpause_reporting_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);

    let result = client.try_unpause_reporting(&admin);
    assert!(result.is_err(), "unpause_reporting must require auth; succeeded without it");
}

#[test]
fn test_unpause_reporting_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    let _ = env.auths();

    client.unpause_reporting(&admin).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
    assert!(!client.is_reporting_paused(), "contract should be unpaused");
}

#[test]
fn test_transfer_ownership_rejected_without_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    let new_owner = Address::generate(&env);

    let result = client.try_transfer_ownership(&admin, &new_owner);
    assert!(result.is_err(), "transfer_ownership must require auth; succeeded without it");
}

#[test]
fn test_transfer_ownership_accepted_with_admin_auth() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let new_owner = Address::generate(&env);

    client.transfer_ownership(&admin, &new_owner).unwrap();

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, admin, "admin must be the authorised address");
    assert_eq!(client.admin(), new_owner, "admin should have changed to new_owner");
}

#[test]
fn test_transfer_ownership_self_transfer_rejected() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    let result = client.try_transfer_ownership(&admin, &admin);
    assert!(result.is_err(), "self-transfer must be rejected");
    match result.err().unwrap() {
        Ok(e) => assert_eq!(e, ReportingError::InvalidNewOwner),
        Err(_) => panic!("expected InvalidNewOwner contract error, got host error"),
    }
}

#[test]
fn test_transfer_ownership_new_owner_is_stored() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let new_owner = Address::generate(&env);

    client.transfer_ownership(&admin, &new_owner).unwrap();
    let _ = env.auths();

    assert_eq!(client.admin(), new_owner, "new_owner should be the stored admin after transfer");
}

#[test]
fn test_is_reporting_paused_does_not_require_auth() {
    let env = Env::default();
    env.ledger().set(ledger_info());
    let contract = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(&env, &contract);

    let paused = client.is_reporting_paused();
    assert!(!paused, "freshly registered contract should not be paused");
}

#[test]
fn test_admin_view_returns_current_admin() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    assert_eq!(client.admin(), admin, "admin view should return the admin");
}

#[test]
fn test_state_changing_ops_succeed_after_unpause() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);
    let reporter = Address::generate(&env);

    client.pause_reporting(&admin).unwrap();
    client.unpause_reporting(&admin).unwrap();
    let _ = env.auths();

    let result =
        client.try_submit_report(&reporter, &1u32, &report_data(&env), &report_hash(&env));
    match result {
        Ok(_) => {}
        Err(e) => match e {
            Ok(contract_err) => assert_ne!(
                contract_err,
                ReportingError::ReportingPaused,
                "should not get ReportingPaused after unpause"
            ),
            Err(_) => panic!("unexpected host error after unpause"),
        },
    }
}

#[test]
fn test_double_pause_is_idempotent() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    client.pause_reporting(&admin).unwrap();

    assert!(client.is_reporting_paused(), "should still be paused");
}

#[test]
fn test_double_unpause_is_idempotent() {
    let env = Env::default();
    let (client, admin) = register_and_init(&env);

    client.pause_reporting(&admin).unwrap();
    client.unpause_reporting(&admin).unwrap();
    client.unpause_reporting(&admin).unwrap();

    assert!(!client.is_reporting_paused(), "should remain unpaused");
}
