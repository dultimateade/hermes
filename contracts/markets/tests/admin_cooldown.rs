#![cfg(test)]

extern crate std;

use markets::admin::AdminManager;
use markets::errors::ContractError;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    Address, Env, Symbol,
};

#[contract]
pub struct DummyContract;

#[contractimpl]
impl DummyContract {
    pub fn set_cd(env: Env, admin: Address, secs: u64) -> Result<(), ContractError> {
        AdminManager::set_admin_cooldown(&env, &admin, secs)
    }

    pub fn get_cd(env: Env) -> u64 {
        AdminManager::get_admin_cooldown(&env)
    }

    pub fn check_cd(env: Env, admin: Address, func: Symbol) -> Result<(), ContractError> {
        AdminManager::check_admin_cooldown(&env, &admin, &func)
    }

    pub fn add_admin(env: Env, admin: Address) -> Result<(), ContractError> {
        AdminManager::add_admin(&env, &admin)
    }

    pub fn get_admins(env: Env) -> soroban_sdk::Vec<Address> {
        AdminManager::get_admins(&env)
    }
}

#[test]
fn test_admin_cooldown() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DummyContract, ());
    let client = DummyContractClient::new(&env, &contract_id);

    // By default, no cooldown
    assert_eq!(client.get_cd(), 0);

    let admin = Address::generate(&env);

    // Set a cooldown of 300 seconds (5 minutes)
    client.set_cd(&admin, &300);
    assert_eq!(client.get_cd(), 300);

    let func_name = Symbol::new(&env, "some_admin_action");

    // Set a baseline timestamp > 0
    env.ledger().with_mut(|l| {
        l.timestamp = 1000;
    });

    // First invocation should succeed since last action is 0
    let res = client.try_check_cd(&admin, &func_name);
    assert!(res.is_ok());

    // Second invocation immediately should fail due to cooldown
    let res_err = client.try_check_cd(&admin, &func_name);
    assert_eq!(
        res_err.unwrap_err().unwrap(),
        ContractError::AdminCooldownActive
    );

    // Advance ledger timestamp beyond cooldown
    env.ledger().with_mut(|l| {
        l.timestamp += 301;
    });

    // Invocation should now succeed
    let res_after = client.try_check_cd(&admin, &func_name);
    assert!(res_after.is_ok());
}

#[test]
fn test_cooldown_disabled() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DummyContract, ());
    let client = DummyContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.set_cd(&admin, &0);

    let func_name = Symbol::new(&env, "action2");

    // Should succeed consecutively
    assert!(client.try_check_cd(&admin, &func_name).is_ok());
    assert!(client.try_check_cd(&admin, &func_name).is_ok());
}

#[test]
fn test_add_admin_rejects_duplicate() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DummyContract, ());
    let client = DummyContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // First add succeeds
    assert!(client.try_add_admin(&admin).is_ok());
    let len_after_first = client.get_admins().len();
    assert_eq!(len_after_first, 1);

    // Second add with the same address must fail
    let res = client.try_add_admin(&admin);
    let err = res.unwrap_err().unwrap();
    assert!(
        err == ContractError::AdminAddressInvalid
            || err == ContractError::AdminAlreadyExists,
        "expected duplicate admin rejection, got {:?}",
        err
    );

    // Admin list length must be unchanged
    assert_eq!(client.get_admins().len(), len_after_first);
}
