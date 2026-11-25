use markets::{MarketsContract, MarketsContractClient};
use resolution::{
    errors::ContractError, ResolutionContract, ResolutionContractClient, ResolutionStatus,
};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger as _},
    Address, Env, Vec,
};

#[contract]
struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn set_result(env: Env, market_id: u32, result: Option<Vec<u32>>) {
        env.storage().persistent().set(&market_id, &result);
    }

    pub fn get_result(env: Env, market_id: u32) -> Option<Vec<u32>> {
        env.storage().persistent().get(&market_id)
    }
}

struct Fixture {
    env: Env,
    resolver: Address,
    admin: Address,
    resolution_id: Address,
    markets_id: Address,
    oracle_id: Address,
}

impl Fixture {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let resolver = Address::generate(&env);

        let markets_id = env.register(MarketsContract, ());
        let resolution_id = env.register(ResolutionContract, ());
        let oracle_id = env.register(MockOracle, ());

        let resolution = ResolutionContractClient::new(&env, &resolution_id);
        resolution.initialize(&admin);
        resolution.set_integrations(&admin, &markets_id, &oracle_id);

        Self {
            env,
            resolver,
            admin,
            resolution_id,
            markets_id,
            oracle_id,
        }
    }

    fn resolution(&self) -> ResolutionContractClient<'_> {
        ResolutionContractClient::new(&self.env, &self.resolution_id)
    }

    fn markets(&self) -> MarketsContractClient<'_> {
        MarketsContractClient::new(&self.env, &self.markets_id)
    }

    fn create_market(&self) -> u32 {
        self.create_market_ending_at(self.env.ledger().timestamp())
    }

    fn create_market_ending_at(&self, end_time: u64) -> u32 {
        let outcomes = Vec::from_array(
            &self.env,
            [
                soroban_sdk::String::from_str(&self.env, "yes"),
                soroban_sdk::String::from_str(&self.env, "no"),
                soroban_sdk::String::from_str(&self.env, "maybe"),
            ],
        );
        self.markets().create_market(
            &self.resolver,
            &soroban_sdk::String::from_str(&self.env, "Will it happen?"),
            &soroban_sdk::String::from_str(&self.env, "Test market"),
            &end_time,
            &soroban_sdk::String::from_str(&self.env, "oracle"),
            &outcomes,
        )
    }

    fn set_oracle_result(&self, market_id: u32, result: Option<Vec<u32>>) {
        let oracle = MockOracleClient::new(&self.env, &self.oracle_id);
        oracle.set_result(&market_id, &result);
    }
}

#[test]
fn resolves_market_and_persists_status_and_all_winning_outcomes() {
    let fixture = Fixture::new();
    let market_id = fixture.create_market();
    fixture.set_oracle_result(market_id, Some(Vec::from_array(&fixture.env, [0, 2])));

    let status = fixture
        .resolution()
        .resolve_market(&fixture.resolver, &market_id);

    assert_eq!(
        status,
        ResolutionStatus {
            market_id,
            winning_outcomes: Vec::from_array(&fixture.env, [0, 2]),
            resolved_at: fixture.env.ledger().timestamp(),
            oracle: fixture.oracle_id,
        }
    );
    assert_eq!(
        fixture.resolution().get_resolution_status(&market_id),
        Some(status)
    );
    assert_eq!(
        fixture.markets().get_winning_outcomes(&market_id),
        Some(Vec::from_array(&fixture.env, [0, 2]))
    );
}

#[test]
fn rejects_missing_oracle_results() {
    let fixture = Fixture::new();
    let market_id = fixture.create_market();
    fixture.set_oracle_result(market_id, None);

    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &market_id),
        Err(Ok(ContractError::OracleResultNotAvailable))
    );
}

#[test]
fn rejects_market_that_has_not_closed() {
    let fixture = Fixture::new();
    let market_id = fixture.create_market_ending_at(fixture.env.ledger().timestamp() + 1);
    fixture.set_oracle_result(market_id, Some(Vec::from_array(&fixture.env, [0])));

    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &market_id),
        Err(Ok(ContractError::MarketClosed))
    );
}

#[test]
fn rejects_unknown_market_and_unauthorized_configuration() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &u32::MAX),
        Err(Ok(ContractError::MarketNotFound))
    );

    let unauthorized_admin = Address::generate(&fixture.env);
    assert_eq!(
        fixture.resolution().try_set_integrations(
            &unauthorized_admin,
            &fixture.markets_id,
            &fixture.oracle_id
        ),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn rejects_non_creator_resolution() {
    let fixture = Fixture::new();
    let market_id = fixture.create_market();
    let other_resolver = Address::generate(&fixture.env);

    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&other_resolver, &market_id),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn rejects_duplicate_and_out_of_range_winners() {
    let fixture = Fixture::new();
    let duplicate_market = fixture.create_market();
    fixture.set_oracle_result(
        duplicate_market,
        Some(Vec::from_array(&fixture.env, [0, 0])),
    );
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &duplicate_market),
        Err(Ok(ContractError::InvalidWinningOutcomes))
    );

    let invalid_market = fixture.create_market();
    fixture.set_oracle_result(invalid_market, Some(Vec::from_array(&fixture.env, [3])));
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &invalid_market),
        Err(Ok(ContractError::InvalidOutcome))
    );

    let empty_market = fixture.create_market();
    fixture.set_oracle_result(empty_market, Some(Vec::new(&fixture.env)));
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &empty_market),
        Err(Ok(ContractError::InvalidWinningOutcomes))
    );
}

#[test]
fn enforces_cooldown_per_resolver() {
    let fixture = Fixture::new();
    fixture
        .resolution()
        .set_resolution_cooldown(&fixture.admin, &60);
    let first_market = fixture.create_market();
    fixture.set_oracle_result(first_market, Some(Vec::from_array(&fixture.env, [0])));
    fixture
        .resolution()
        .resolve_market(&fixture.resolver, &first_market);

    let second_market = fixture.create_market();
    fixture.set_oracle_result(second_market, Some(Vec::from_array(&fixture.env, [1])));
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &second_market),
        Err(Ok(ContractError::ResolutionCooldownActive))
    );
}

#[test]
fn allows_resolution_after_cooldown_expires() {
    let fixture = Fixture::new();
    fixture
        .resolution()
        .set_resolution_cooldown(&fixture.admin, &60);
    let first_market = fixture.create_market();
    fixture.set_oracle_result(first_market, Some(Vec::from_array(&fixture.env, [0])));
    fixture
        .resolution()
        .resolve_market(&fixture.resolver, &first_market);

    let second_market = fixture.create_market();
    fixture.set_oracle_result(second_market, Some(Vec::from_array(&fixture.env, [1])));

    // Advance ledger timestamp beyond cooldown (60 seconds)
    fixture.env.ledger().set_timestamp(fixture.env.ledger().timestamp() + 61);

    let status = fixture
        .resolution()
        .resolve_market(&fixture.resolver, &second_market);
    assert_eq!(status.market_id, second_market);
}

#[test]
fn rejects_resolving_already_resolved_market() {
    let fixture = Fixture::new();
    let market_id = fixture.create_market();
    fixture.set_oracle_result(market_id, Some(Vec::from_array(&fixture.env, [0])));

    fixture
        .resolution()
        .resolve_market(&fixture.resolver, &market_id);

    // Second resolution attempt should fail
    assert_eq!(
        fixture
            .resolution()
            .try_resolve_market(&fixture.resolver, &market_id),
        Err(Ok(ContractError::MarketAlreadyResolved))
    );
}

#[test]
fn rejects_double_initialization() {
    let fixture = Fixture::new();
    let another_admin = Address::generate(&fixture.env);

    assert_eq!(
        fixture.resolution().try_initialize(&another_admin),
        Err(Ok(ContractError::InvalidState))
    );
}

#[test]
fn rejects_unauthorized_cooldown_update() {
    let fixture = Fixture::new();
    let unauthorized_user = Address::generate(&fixture.env);

    assert_eq!(
        fixture
            .resolution()
            .try_set_resolution_cooldown(&unauthorized_user, &100),
        Err(Ok(ContractError::Unauthorized))
    );
}

#[test]
fn returns_correct_version() {
    let fixture = Fixture::new();
    assert_eq!(fixture.resolution().version(), 7);
}

// ── Oracle registry validation tests for resolve_market_with_oracles ────────

/// A mock oracle that implements both `list_oracles` and `get_result`, matching
/// the two entrypoints called by `resolve_market_with_oracles`.
#[contract]
struct MockRegistryOracle;

#[contractimpl]
impl MockRegistryOracle {
    /// Store `result` for `market_id`.
    pub fn set_result(env: Env, market_id: u32, result: Option<Vec<u32>>) {
        env.storage().persistent().set(&market_id, &result);
    }

    /// Return the stored result for `market_id`.
    pub fn get_result(env: Env, market_id: u32) -> Option<Vec<u32>> {
        env.storage().persistent().get(&market_id)
    }

    /// Return the list of addresses stored as registered oracles.
    pub fn list_oracles(env: Env) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&0u32)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Register an oracle address so `list_oracles` returns it.
    pub fn register(env: Env, oracle: Address) {
        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&0u32)
            .unwrap_or_else(|| Vec::new(&env));
        if !list.contains(&oracle) {
            list.push_back(oracle);
        }
        env.storage().persistent().set(&0u32, &list);
    }
}

struct RegistryFixture {
    env: Env,
    resolver: Address,
    admin: Address,
    resolution_id: Address,
    markets_id: Address,
    /// Address of the mock registry oracle contract (plays the role of the oracles contract).
    registry_oracle_id: Address,
    /// A second oracle address registered inside the mock registry.
    registered_oracle: Address,
}

impl RegistryFixture {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let resolver = Address::generate(&env);

        let markets_id = env.register(MarketsContract, ());
        let resolution_id = env.register(ResolutionContract, ());
        // The registry oracle contract acts as the OracleContract stored in the
        // resolution contract. Its `list_oracles` entrypoint is called to validate
        // the oracle address, and its `get_result` entrypoint is called to fetch
        // the market result.
        let registry_oracle_id = env.register(MockRegistryOracle, ());

        // Self-register so the fixture oracle address is in the registry.
        let registry_client = MockRegistryOracleClient::new(&env, &registry_oracle_id);
        registry_client.register(&registry_oracle_id);

        let resolution = ResolutionContractClient::new(&env, &resolution_id);
        resolution.initialize(&admin);
        resolution.set_integrations(&admin, &markets_id, &registry_oracle_id);

        let registered_oracle = registry_oracle_id.clone();

        Self {
            env,
            resolver,
            admin,
            resolution_id,
            markets_id,
            registry_oracle_id,
            registered_oracle,
        }
    }

    fn resolution(&self) -> ResolutionContractClient<'_> {
        ResolutionContractClient::new(&self.env, &self.resolution_id)
    }

    fn markets(&self) -> MarketsContractClient<'_> {
        MarketsContractClient::new(&self.env, &self.markets_id)
    }

    fn create_market(&self) -> u32 {
        let outcomes = Vec::from_array(
            &self.env,
            [
                soroban_sdk::String::from_str(&self.env, "yes"),
                soroban_sdk::String::from_str(&self.env, "no"),
            ],
        );
        self.markets().create_market(
            &self.resolver,
            &soroban_sdk::String::from_str(&self.env, "Will it happen?"),
            &soroban_sdk::String::from_str(&self.env, "Test market"),
            &self.env.ledger().timestamp(),
            &soroban_sdk::String::from_str(&self.env, "oracle"),
            &outcomes,
        )
    }

    fn set_result(&self, market_id: u32, result: Option<Vec<u32>>) {
        MockRegistryOracleClient::new(&self.env, &self.registry_oracle_id)
            .set_result(&market_id, &result);
    }
}

/// A registered oracle address passes the registry check and resolves the market.
#[test]
fn resolve_market_with_oracles_accepts_registered_oracle() {
    let fixture = RegistryFixture::new();
    let market_id = fixture.create_market();
    fixture.set_result(market_id, Some(Vec::from_array(&fixture.env, [0])));

    let status = fixture.resolution().resolve_market_with_oracles(
        &fixture.resolver,
        &market_id,
        &fixture.registered_oracle,
    );

    assert_eq!(status.market_id, market_id);
    assert_eq!(
        status.winning_outcomes,
        Vec::from_array(&fixture.env, [0])
    );
    assert_eq!(status.oracle, fixture.registered_oracle);
    assert_eq!(
        fixture.markets().get_winning_outcomes(&market_id),
        Some(Vec::from_array(&fixture.env, [0]))
    );
}

/// An address that is NOT in the oracle registry is rejected with `OracleNotRegistered`.
#[test]
fn resolve_market_with_oracles_rejects_unregistered_oracle() {
    let fixture = RegistryFixture::new();
    let market_id = fixture.create_market();
    fixture.set_result(market_id, Some(Vec::from_array(&fixture.env, [0])));

    let fake_oracle = Address::generate(&fixture.env);

    assert_eq!(
        fixture.resolution().try_resolve_market_with_oracles(
            &fixture.resolver,
            &market_id,
            &fake_oracle,
        ),
        Err(Ok(ContractError::OracleNotRegistered))
    );
}

/// When the oracle is registered but returns no result, `OracleResultNotAvailable` is returned.
#[test]
fn resolve_market_with_oracles_rejects_missing_oracle_result() {
    let fixture = RegistryFixture::new();
    let market_id = fixture.create_market();
    fixture.set_result(market_id, None);

    assert_eq!(
        fixture.resolution().try_resolve_market_with_oracles(
            &fixture.resolver,
            &market_id,
            &fixture.registered_oracle,
        ),
        Err(Ok(ContractError::OracleResultNotAvailable))
    );
}

// ── TTL validation at initialization ────────────────────────────────────────

/// Helper: register a fresh ResolutionContract and attempt to initialize it
/// against a ledger whose `max_entry_ttl` is set to `max_ttl`.
fn try_initialize_with_max_ttl(max_ttl: u32) -> Result<(), soroban_sdk::Error<ContractError>> {
    let env = Env::default();
    env.mock_all_auths();

    // Override the ledger's max_entry_ttl so env.storage().max_ttl() returns
    // exactly `max_ttl` during the initialize call.
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: 0,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: max_ttl,
    });

    let resolution_id = env.register(ResolutionContract, ());
    let resolution = ResolutionContractClient::new(&env, &resolution_id);
    let admin = Address::generate(&env);
    resolution.try_initialize(&admin)
}

/// When `max_ttl` equals or exceeds the larger of the two constants
/// (`RESOLUTION_TTL_LEDGERS = 6_307_200`), initialization must succeed
/// and no warning event is emitted.
#[test]
fn initialize_succeeds_when_max_ttl_is_sufficient() {
    // 6_307_200 is the larger constant; use exactly that value.
    assert_eq!(try_initialize_with_max_ttl(6_307_200), Ok(()));
}

/// When `max_ttl` is set below `COOLDOWN_TTL_LEDGERS` (535_680) both
/// constants exceed the limit.  `initialize` must return
/// `TtlExceedsNetworkMax` and emit warning events for both constants.
#[test]
fn initialize_fails_and_emits_events_when_both_ttls_exceed_max() {
    // A testnet-like cap that is smaller than COOLDOWN_TTL_LEDGERS (535_680).
    let low_max_ttl: u32 = 100_000;

    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: 0,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: low_max_ttl,
    });

    let resolution_id = env.register(ResolutionContract, ());
    let resolution = ResolutionContractClient::new(&env, &resolution_id);
    let admin = Address::generate(&env);

    let result = resolution.try_initialize(&admin);
    assert_eq!(result, Err(Ok(ContractError::TtlExceedsNetworkMax)));

    // Both warning events must have been emitted (one per exceeding constant).
    let events = env.events().all();
    assert!(
        events.len() >= 2,
        "expected at least 2 ttl_warn events, got {}",
        events.len()
    );
}

/// When `max_ttl` is between the two constants (above `COOLDOWN_TTL_LEDGERS`
/// but below `RESOLUTION_TTL_LEDGERS`), only `resolution_ttl` exceeds the cap.
/// `initialize` must still return `TtlExceedsNetworkMax`.
#[test]
fn initialize_fails_when_only_resolution_ttl_exceeds_max() {
    // Above COOLDOWN_TTL_LEDGERS (535_680) but below RESOLUTION_TTL_LEDGERS (6_307_200).
    let mid_max_ttl: u32 = 1_000_000;
    assert_eq!(
        try_initialize_with_max_ttl(mid_max_ttl),
        Err(Ok(ContractError::TtlExceedsNetworkMax))
    );
}

/// A second `initialize` call on an already-initialized contract returns
/// `InvalidState`, not `TtlExceedsNetworkMax`, regardless of the max_ttl.
/// This confirms the duplicate-initialization guard fires before the TTL check.
#[test]
fn second_initialize_returns_invalid_state_not_ttl_error() {
    // Use a sufficient max_ttl so the first initialize succeeds.
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: 0,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 6_307_200,
    });

    let resolution_id = env.register(ResolutionContract, ());
    let resolution = ResolutionContractClient::new(&env, &resolution_id);
    let admin = Address::generate(&env);

    resolution.initialize(&admin);

    // Second attempt: even with a low cap, InvalidState fires first.
    env.ledger().set(soroban_sdk::testutils::LedgerInfo {
        timestamp: 0,
        protocol_version: 20,
        sequence_number: 0,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl: 1,
    });
    assert_eq!(
        resolution.try_initialize(&admin),
        Err(Ok(ContractError::InvalidState))
    );
}
