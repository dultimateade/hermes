use oracles::{Error, OraclesContract, OraclesContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

#[test]
fn returns_results_only_when_registered_oracles_agree() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let oracle_a = Address::generate(&env);
    let oracle_b = Address::generate(&env);
    let contract_id = env.register(OraclesContract, ());
    let client = OraclesContractClient::new(&env, &contract_id);

    client.add_oracle(&admin, &oracle_a);
    client.add_oracle(&admin, &oracle_b);
    assert_eq!(client.get_result(&7), None);

    let result = Vec::from_array(&env, [0, 2]);
    client.submit_market_result(&oracle_a, &7, &result);
    assert_eq!(client.get_result(&7), Some(result.clone()));

    client.submit_market_result(&oracle_b, &7, &Vec::from_array(&env, [1]));
    assert_eq!(client.get_result(&7), None);

    client.submit_market_result(&oracle_b, &7, &Vec::from_array(&env, [2, 0]));
    assert_eq!(client.get_result(&7), Some(result));
}

#[test]
fn rejects_results_from_unregistered_oracles() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(OraclesContract, ());
    let client = OraclesContractClient::new(&env, &contract_id);
    let unregistered = Address::generate(&env);

    assert_eq!(
        client.try_submit_market_result(&unregistered, &1, &Vec::from_array(&env, [0])),
        Err(Ok(Error::InvalidOracleConfig))
    );
}
