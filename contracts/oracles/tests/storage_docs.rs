const STORAGE_DOCUMENTATION: &str = include_str!("../docs/storage.md");

#[test]
fn storage_documentation_covers_storage_tiers_and_result_ttl() {
    for required_section in ["Instance", "Persistent", "Temporary"] {
        assert!(
            STORAGE_DOCUMENTATION.contains(required_section),
            "storage documentation must describe the {required_section} tier"
        );
    }

    assert!(
        STORAGE_DOCUMENTATION.contains("Symbol(\"OracleList\")"),
        "storage documentation must identify the OracleList key"
    );
    assert!(
        STORAGE_DOCUMENTATION.contains("Vec<Address>"),
        "storage documentation must describe the registry value"
    );
    assert!(
        STORAGE_DOCUMENTATION.contains("DataKey::MarketResult(market_id, oracle_address)"),
        "storage documentation must describe the MarketResult key structure"
    );
    assert!(
        STORAGE_DOCUMENTATION.contains("6_307_200"),
        "storage documentation must specify the one-year market result TTL"
    );
    assert!(
        STORAGE_DOCUMENTATION.contains("submit_market_result")
            && STORAGE_DOCUMENTATION.contains("get_result"),
        "storage documentation must describe market result submission and reads"
    );
}
