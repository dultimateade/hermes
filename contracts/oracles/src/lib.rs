#![no_std]

//! Predictify oracle registry contract.
//!
//! # Entrypoints
//!
//! ## State-changing (require admin `require_auth`)
//! - [`OraclesContract::initialize`] — configure the price staleness threshold
//! - [`OraclesContract::add_oracle`] — register an oracle address
//! - [`OraclesContract::remove_oracle`] — deregister an oracle address
//!
//! ## Read-only (no auth)
//! - [`OraclesContract::capabilities`] — `u64` bitmap of supported features
//! - [`OraclesContract::version`] — contract version number
//! - [`OraclesContract::list_oracles`] — enumerate registered oracles
//! - [`OraclesContract::get_price`] — fetch a raw price from an oracle
//! - [`OraclesContract::get_price_data`] — fetch full price data from an oracle
//! - [`OraclesContract::is_oracle_healthy`] — check liveness and price freshness
//! - [`OraclesContract::check_oracle_health`] — check liveness and feed readiness
//! - [`OraclesContract::submit_market_result`] — submit an authenticated market result
//! - [`OraclesContract::get_result`] — return a unanimous submitted market result

pub mod views;

pub use views::CapabilityFlag;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, IntoVal,
    String, Symbol, Vec,
};

/// Persistent storage key for the oracle address list.
#[contracttype]
pub enum DataKey {
    /// Stores the address that initialized the contract.
    Admin,
    /// Stores `Vec<Address>` — the ordered list of registered oracle addresses.
    OracleList,
    /// Stores a market outcome `String` for the `(market_id, oracle_address)` pair.
    MarketResult(Symbol, Address),
}

/// Price data returned by `get_price_data`.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OraclePriceData {
    /// The asset price as a fixed-point integer.
    pub price: i128,
    /// Unix timestamp of the price publication.
    pub publish_time: u64,
    /// Optional confidence interval around the price.
    pub confidence: Option<i128>,
    /// Decimal exponent.
    pub exponent: i32,
}

/// Contract-level error codes.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// The oracle is unavailable or does not advertise the required capability.
    OracleUnavailable = 200,
    /// The oracle address is not registered in the registry.
    InvalidOracleConfig = 201,
    /// The oracle data is too old to be trusted.
    OracleStale = 202,
    /// Multiple oracles disagree and no consensus was reached.
    OracleNoConsensus = 203,
    /// The oracle data has been successfully verified.
    OracleVerified = 204,
    /// The market is not yet ready for oracle queries.
    MarketNotReady = 205,
    /// The fallback oracle is also unavailable.
    FallbackOracleUnavailable = 206,
    /// The resolution timeout has been reached.
    ResolutionTimeoutReached = 207,
    /// The oracle's confidence interval is too wide.
    OracleConfidenceTooWide = 208,
    /// The oracle feed identifier is invalid.
    InvalidOracleFeed = 209,
    /// The oracle callback authorization failed.
    OracleCallbackAuthFailed = 210,
    /// The oracle callback caller is not authorized.
    OracleCallbackUnauthorized = 211,
    /// The oracle callback signature is invalid.
    OracleCallbackInvalidSignature = 212,
    /// A replayed oracle callback was detected.
    OracleCallbackReplayDetected = 213,
    /// The oracle callback arrived after its deadline.
    OracleCallbackTimeout = 214,
    /// The contract has already been initialized.
    AlreadyInitialized = 215,
    // ===== CAPABILITIES ERRORS (220-229) =====
    /// The requested capability is not supported by this contract version.
    CapabilityNotSupported = 220,
    /// The capabilities query returned an unexpected or malformed bitmap.
    CapabilityBitmapCorrupt = 221,
    /// A reserved capability bit was unexpectedly set.
    ReservedCapabilitySet = 222,
}

/// Minimum TTL ledgers for the oracle registry key.
const REGISTRY_TTL_BUMP_THRESHOLD: u32 = 120_960;
/// Extended TTL ledgers for the oracle registry key.
const REGISTRY_TTL_BUMP_TO: u32 = 518_400;
/// Maximum age in seconds for an oracle price publication.
const MAX_PRICE_STALENESS_SECONDS: u64 = 3_600;

fn same_outcomes(env: &Env, left: &Vec<u32>, right: &Vec<u32>) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut outcomes = Map::<u32, bool>::new(env);
    for outcome in left.iter() {
        outcomes.set(outcome, true);
    }
    if outcomes.len() != left.len() {
        return false;
    }
    right.iter().all(|outcome| outcomes.contains_key(outcome))
}

/// One year of ledgers at an average ledger close time of five seconds.
const MARKET_RESULT_TTL_LEDGERS: u32 = 6_307_200;
/// Renew market result entries when fewer than seven days remain.
const MARKET_RESULT_TTL_BUMP_THRESHOLD: u32 = 120_960;

/// Bump the TTL of the oracle list on hot storage access without writing data.
fn bump_registry_ttl(env: &Env) {
    if env.storage().persistent().has(&DataKey::OracleList) {
        env.storage().persistent().extend_ttl(
            &DataKey::OracleList,
            REGISTRY_TTL_BUMP_THRESHOLD,
            REGISTRY_TTL_BUMP_TO,
        );
    }
}

/// Load the oracle list, renewing its persistent storage TTL first.
fn load_oracle_list(env: &Env) -> Vec<Address> {
    bump_registry_ttl(env);
    env.storage()
        .persistent()
        .get(&DataKey::OracleList)
        .unwrap_or_else(|| Vec::new(env))
}

fn max_price_staleness_seconds(env: &Env) -> u64 {
    env.storage()
        .instance()
        .extend_ttl(REGISTRY_TTL_BUMP_THRESHOLD, REGISTRY_TTL_BUMP_TO);
    env.storage()
        .instance()
        .get(&DataKey::MaxPriceStalenessSeconds)
        .unwrap_or(MAX_PRICE_STALENESS_SECONDS)
}

fn oracle_is_live(env: &Env, oracle: &Address) -> bool {
    let live: Option<bool> =
        env.invoke_contract(oracle, &symbol_short!("is_live"), soroban_sdk::vec![env]);
    live.unwrap_or(false)
}

/// Persist the oracle list and bump the entry's TTL.
fn save_oracle_list(env: &Env, list: &Vec<Address>) {
    env.storage().persistent().set(&DataKey::OracleList, list);
    env.storage().persistent().extend_ttl(
        &DataKey::OracleList,
        REGISTRY_TTL_BUMP_THRESHOLD,
        REGISTRY_TTL_BUMP_TO,
    );
}

fn bump_market_result_ttl(env: &Env, key: &DataKey) {
    env.storage().persistent().extend_ttl(
        key,
        MARKET_RESULT_TTL_BUMP_THRESHOLD,
        MARKET_RESULT_TTL_LEDGERS,
    );
}

#[contract]
pub struct OraclesContract;

#[contractimpl]
impl OraclesContract {
    /// Configure the maximum accepted price age.
    ///
    /// Requires `admin` authorization and can only be called once. Until
    /// initialized, the contract uses the default one-hour threshold.
    pub fn initialize(env: Env, admin: Address, max_staleness_seconds: u64) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::MaxPriceStalenessSeconds, &max_staleness_seconds);
        env.storage()
            .instance()
            .extend_ttl(REGISTRY_TTL_BUMP_THRESHOLD, REGISTRY_TTL_BUMP_TO);
        Ok(())
    }

    /// Return a bitmap of features supported by this contract build.
    pub fn capabilities(_env: Env) -> u64 {
        views::capabilities()
    }

    /// Return the contract's numeric version.
    pub fn version(_env: Env) -> u32 {
        7
    }

    /// Return all registered oracle addresses and renew the registry TTL.
    pub fn list_oracles(env: Env) -> Vec<Address> {
        load_oracle_list(&env)
    }

    /// Store a market outcome for `oracle_address`.
    ///
    /// Requires authorization from `oracle_address`. The persistent result
    /// entry is given a one-year TTL.
    pub fn submit_market_result(
        env: Env,
        market_id: Symbol,
        oracle_address: Address,
        result: String,
    ) {
        oracle_address.require_auth();

        let key = DataKey::MarketResult(market_id, oracle_address);
        env.storage().persistent().set(&key, &result);
        env.storage().persistent().extend_ttl(
            &key,
            MARKET_RESULT_TTL_LEDGERS,
            MARKET_RESULT_TTL_LEDGERS,
        );
    }

    /// Return a stored market outcome, renewing its TTL when fewer than seven
    /// days remain.
    pub fn get_result(env: Env, market_id: Symbol, oracle_address: Address) -> Option<String> {
        let key = DataKey::MarketResult(market_id, oracle_address);
        let result = env.storage().persistent().get(&key);
        if result.is_some() {
            bump_market_result_ttl(&env, &key);
        }
        result
    }

    /// Register `oracle` in the registry.
    ///
    /// Requires `admin` authorization and the oracle's `capabilities()` bitmap
    /// to include `CapabilityFlag::GET_PRICE`.
    pub fn add_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), Error> {
        admin.require_auth();

        let capabilities = env.try_invoke_contract::<u64, soroban_sdk::Error>(
            &oracle,
            &symbol_short!("capabilities"),
            Vec::new(&env),
        );
        if !matches!(
            capabilities,
            Ok(Ok(caps)) if caps & CapabilityFlag::GET_PRICE != 0
        ) {
            return Err(Error::OracleUnavailable);
        }

        let mut list = load_oracle_list(&env);
        if !list.contains(&oracle) {
            list.push_back(oracle);
        }
        save_oracle_list(&env, &list);
        Ok(())
    }

    /// Remove `oracle` from the registry.
    ///
    /// Requires `admin` authorization.
    pub fn remove_oracle(env: Env, admin: Address, oracle: Address) {
        admin.require_auth();

        let list = load_oracle_list(&env);
        let mut filtered: Vec<Address> = Vec::new(&env);
        for a in list.iter() {
            if a != oracle {
                filtered.push_back(a);
            }
        }
        save_oracle_list(&env, &filtered);
    }

    /// Fetch a raw price from a registered oracle.
    pub fn get_price(env: Env, oracle: Address, feed_id: String) -> Result<i128, Error> {
        let list = load_oracle_list(&env);
        if !list.contains(&oracle) {
            return Err(Error::InvalidOracleConfig);
        }

        let price: Option<i128> = env.invoke_contract(
            &oracle,
            &symbol_short!("get_price"),
            soroban_sdk::vec![&env, feed_id.into_val(&env)],
        );
        price.ok_or(Error::OracleUnavailable)
    }

    /// Fetch full price data from a registered oracle, falling back to its
    /// raw price entrypoint when extended data is unavailable.
    pub fn get_price_data(
        env: Env,
        oracle: Address,
        feed_id: String,
    ) -> Result<OraclePriceData, Error> {
        let list = load_oracle_list(&env);
        if !list.contains(&oracle) {
            return Err(Error::InvalidOracleConfig);
        }

        let full: Option<OraclePriceData> = env.invoke_contract(
            &oracle,
            &symbol_short!("get_pdata"),
            soroban_sdk::vec![&env, feed_id.clone().into_val(&env)],
        );
        if let Some(data) = full {
            let now = env.ledger().timestamp();
            if now > data.publish_time
                && now.saturating_sub(data.publish_time) > max_price_staleness_seconds(&env)
            {
                return Err(Error::OracleStale);
            }
            return Ok(data);
        }

        let price: Option<i128> = env.invoke_contract(
            &oracle,
            &symbol_short!("get_price"),
            soroban_sdk::vec![&env, feed_id.into_val(&env)],
        );
        match price {
            Some(_) => Err(Error::OracleStale),
            None => Err(Error::OracleUnavailable),
        }
    }

    /// Check whether a registered oracle is live and returns fresh price data.
    pub fn is_oracle_healthy(
        env: Env,
        oracle: Address,
        feed_id: String,
    ) -> Result<bool, Error> {
        let list = load_oracle_list(&env);
        if !list.contains(&oracle) {
            return Err(Error::InvalidOracleConfig);
        }

        if !oracle_is_live(&env, &oracle) {
            return Ok(false);
        }

        let price_data: Option<OraclePriceData> = env.invoke_contract(
            &oracle,
            &symbol_short!("get_pdata"),
            soroban_sdk::vec![&env, feed_id.into_val(&env)],
        );
        let Some(price_data) = price_data else {
            return Ok(false);
        };

        let now = env.ledger().timestamp();
        Ok(now.saturating_sub(price_data.publish_time) <= max_price_staleness_seconds(&env))
    }

    /// Check provider liveness and verify that a specific feed returns fresh, valid price data.
    pub fn check_oracle_health(env: Env, oracle: Address, feed_id: String) -> Result<(), Error> {
        let list = load_oracle_list(&env);
        if !list.contains(&oracle) {
            return Err(Error::InvalidOracleConfig);
        }
        if !oracle_is_live(&env, &oracle) {
            return Err(Error::OracleUnavailable);
        }

        let price_data = Self::get_price_data(env, oracle, feed_id)?;
        if price_data.price <= 0 {
            return Err(Error::OracleUnavailable);
        }

        Ok(())
    }

    /// Submit a market result on behalf of a registered oracle.
    ///
    /// Each registered oracle has an independent submission. A provider may
    /// update its own result until resolution; results are exposed only when
    /// all submitted values agree.
    pub fn submit_market_result(
        env: Env,
        oracle: Address,
        market_id: u32,
        winning_outcomes: Vec<u32>,
    ) -> Result<(), Error> {
        oracle.require_auth();
        let oracle_list = load_oracle_list(&env);
        if !oracle_list.contains(&oracle) {
            return Err(Error::InvalidOracleConfig);
        }

        let key = DataKey::MarketResult(market_id, oracle);
        env.storage().persistent().set(&key, &winning_outcomes);
        env.storage().persistent().extend_ttl(
            &key,
            REGISTRY_TTL_BUMP_THRESHOLD.min(env.storage().max_ttl()),
            REGISTRY_TTL_BUMP_TO.min(env.storage().max_ttl()),
        );
        Ok(())
    }

    /// Return the market result if all registered submissions agree.
    pub fn get_result(env: Env, market_id: u32) -> Option<Vec<u32>> {
        let oracle_list = load_oracle_list(&env);
        let mut consensus: Option<Vec<u32>> = None;

        for oracle in oracle_list.iter() {
            let key = DataKey::MarketResult(market_id, oracle);
            let result: Option<Vec<u32>> = env.storage().persistent().get(&key);
            if let Some(result) = result {
                env.storage().persistent().extend_ttl(
                    &key,
                    REGISTRY_TTL_BUMP_THRESHOLD.min(env.storage().max_ttl()),
                    REGISTRY_TTL_BUMP_TO.min(env.storage().max_ttl()),
                );
                if let Some(existing) = consensus.as_ref() {
                    if !same_outcomes(&env, existing, &result) {
                        return None;
                    }
                } else {
                    consensus = Some(result);
                }
            }
        }

        consensus
    }
}
