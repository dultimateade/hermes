#![cfg(test)]

use admin::{AdminContract, AdminContractClient, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

fn setup(env: &Env) -> (AdminContractClient<'_>, Address) {
    env.mock_all_auths();
    let contract_id = env.register(AdminContract, ());
    let client = AdminContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    (client, admin)
}

#[test]
fn initialization_sets_admin_and_rejects_reinitialization() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    client.initialize(&admin);
    assert_eq!(client.admin(), Ok(admin.clone()));

    let second_admin = Address::generate(&env);
    assert_eq!(
        client.try_initialize(&second_admin),
        Err(Ok(ContractError::AlreadyInitialized))
    );
}

#[test]
fn ownership_transfer_updates_admin_and_revokes_old_admin() {
    let env = Env::default();
    let (client, old_admin) = setup(&env);
    let new_admin = Address::generate(&env);
    let function_name = Symbol::new(&env, "action");

    client.initialize(&old_admin);
    client.propose_ownership(&old_admin, &new_admin);
    assert_eq!(client.admin(), Ok(old_admin.clone()));

    let pending = client.pending_ownership().expect("proposal should be stored");
    assert_eq!(pending.new_admin, new_admin);
    assert_eq!(
        pending.accept_after,
        pending.proposed_at + admin::OWNERSHIP_TRANSFER_TIMELOCK_SECONDS
    );

    assert_eq!(
        client.try_accept_ownership(&new_admin),
        Err(Ok(ContractError::OwnershipTransferTimelocked))
    );

    env.ledger()
        .with_mut(|ledger| ledger.timestamp = pending.accept_after);
    client.accept_ownership(&new_admin);
    assert_eq!(client.admin(), Ok(new_admin.clone()));
    assert!(client.pending_ownership().is_none());

    assert_eq!(
        client.try_set_admin_cooldown(&old_admin, &300),
        Err(Ok(ContractError::Unauthorized))
    );
    assert_eq!(
        client.try_check_admin_cooldown(&old_admin, &function_name),
        Err(Ok(ContractError::Unauthorized))
    );

    client.set_admin_cooldown(&new_admin, &300);
    assert_eq!(client.get_admin_cooldown(), 300);
    assert_eq!(
        client.try_check_admin_cooldown(&new_admin, &function_name),
        Ok(Ok(()))
    );
}

#[test]
fn ownership_transfer_rejects_self_transfer() {
    let env = Env::default();
    let (client, admin) = setup(&env);

    client.initialize(&admin);
    assert_eq!(
        client.try_propose_ownership(&admin, &admin),
        Err(Ok(ContractError::InvalidNewOwner))
    );
    assert_eq!(client.admin(), Ok(admin));
}
