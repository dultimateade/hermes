use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String,
};

use reporting::{ReportingContract, ReportingContractClient};

pub fn ledger_info() -> LedgerInfo {
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

pub fn register(env: &Env) -> (ReportingContractClient, Address) {
    env.ledger().set(ledger_info());
    let contract_id = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    (client, admin)
}

pub fn register_and_init(env: &Env) -> (ReportingContractClient, Address) {
    env.mock_all_auths();
    env.ledger().set(ledger_info());
    let contract_id = env.register_contract(None, ReportingContract);
    let client = ReportingContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin).unwrap();
    let _ = env.auths();
    (client, admin)
}

pub fn report_data(env: &Env) -> String {
    String::from_str(env, "Test report data")
}

pub fn report_hash(env: &Env) -> String {
    String::from_str(env, "0xdeadbeef1234567890abcdef")
}

pub fn dispute_reason(env: &Env) -> String {
    String::from_str(env, "Incorrect data in report")
}
