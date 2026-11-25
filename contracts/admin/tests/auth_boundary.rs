#![cfg(test)]

use admin::{AdminContract, AdminContractClient, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

fn setup(env: &Env) -> (AdminContractClient<'_>, Address) {
    env.mock_all_auths();
    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (client, admin)
}

#[test]
fn initialize_requires_auth() {
    let env = Env::default();
    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    assert!(client.try_initialize(&admin).is_err());
}

#[test]
fn set_admin_cooldown_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_set_admin_cooldown(&stranger, &300),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn disable_admin_cooldown_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_disable_admin_cooldown(&stranger),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn check_admin_cooldown_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);
    let function_name = Symbol::new(&env, "action");

    assert_eq!(
        client.try_check_admin_cooldown(&stranger, &function_name),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn transfer_ownership_rejects_unauthorized_caller() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);
    let new_owner = Address::generate(&env);

    assert_eq!(
        client.try_propose_ownership(&stranger, &new_owner),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn accept_ownership_rejects_unproposed_and_wrong_candidate() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);

    assert_eq!(
        client.try_accept_ownership(&stranger),
        Err(Ok(ContractError::OwnershipTransferNotProposed))
    );
}

#[test]
fn read_only_entrypoints_remain_available() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    assert_eq!(client.admin(), Ok(admin));
    assert_eq!(client.get_admin_cooldown(), 0);
}
