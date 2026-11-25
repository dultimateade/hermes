#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, symbol_short, Address, Env, IntoVal,
    Map, Symbol, Vec,
};

pub mod errors;
pub use errors::ContractError;

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

/// Persistent-storage keys used by the Resolution contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Whether the contract has been initialized.
    Initialized,
    /// The admin address set at initialization.
    Admin,
    /// The address of the trusted MarketsContract.
    MarketsContract,
    /// The `version()` value returned by the MarketsContract at initialization.
    /// Every resolution call re-validates the live version against this value.
    ExpectedMarketsVersion,
}

// ---------------------------------------------------------------------------
// Cross-contract helper
// ---------------------------------------------------------------------------

/// Call `version()` on the contract at `address` and return the result.
///
/// Returns `Err(ContractError::MarketsContractInvalid)` if the call panics or
/// the returned value cannot be decoded as `u32`.
fn call_markets_version(env: &Env, address: &Address) -> Result<u32, ContractError> {
    let result = env.try_invoke_contract::<u32, ContractError>(
        address,
        &symbol_short!("version"),
        soroban_sdk::vec![env],
    );

    match result {
        Ok(Ok(v)) => Ok(v),
        // Any error — host panic, wrong type, or contract error — is treated
        // as a misconfigured address.
        _ => Err(ContractError::MarketsContractInvalid),
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

use errors::ContractError;

const COOLDOWN_TTL_LEDGERS: u32 = 535_680;
const RESOLUTION_TTL_LEDGERS: u32 = 6_307_200;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    MarketsContract,
    OracleContract,
    ResolutionCooldownSeconds,
    LastResolution(Address),
    ResolutionStatus(u32),
}

/// Status returned by resolution queries and successful resolutions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionStatus {
    pub market_id: u32,
    pub winning_outcomes: Vec<u32>,
    pub resolved_at: u64,
    pub oracle: Address,
}

#[contract]
pub struct ResolutionContract;

impl ResolutionContract {
    /// Checks both TTL constants against the network's `max_ttl` at initialize time.
    ///
    /// When a constant exceeds `max_ttl` the Soroban host would silently clamp the
    /// actual TTL, causing earlier-than-expected expiry without any observable signal.
    /// This helper makes the mismatch explicit: it returns
    /// `Err(ContractError::TtlExceedsNetworkMax)` so callers can decide whether to
    /// abort initialization or accept the clamped values.  An event is also emitted so
    /// off-chain monitors can detect the condition in historical ledger data.
    ///
    /// The event schema is:
    ///   topics:  [`"ttl_warn"`, `"<constant_name>"`]
    ///   data:    `(desired: u32, effective: u32)`
    fn validate_ttl_config(env: &Env) -> Result<(), ContractError> {
        let max = env.storage().max_ttl();
        let mut exceeded = false;

        if COOLDOWN_TTL_LEDGERS > max {
            let effective = max;
            env.events().publish(
                (symbol_short!("ttl_warn"), Symbol::new(env, "cooldown_ttl")),
                (COOLDOWN_TTL_LEDGERS, effective),
            );
            exceeded = true;
        }

        if RESOLUTION_TTL_LEDGERS > max {
            let effective = max;
            env.events().publish(
                (symbol_short!("ttl_warn"), Symbol::new(env, "resolution_ttl")),
                (RESOLUTION_TTL_LEDGERS, effective),
            );
            exceeded = true;
        }

        if exceeded {
            return Err(ContractError::TtlExceedsNetworkMax);
        }

        Ok(())
    }

    fn extend_ttl(env: &Env, key: &DataKey, desired_ttl: u32) {
        let ttl = desired_ttl.min(env.storage().max_ttl());
        env.storage().persistent().extend_ttl(key, ttl, ttl);
    }

    fn require_admin(env: &Env, admin: &Address) {
        admin.require_auth();
        if env.storage().persistent().has(&DataKey::Admin) {
            Self::extend_ttl(env, &DataKey::Admin, COOLDOWN_TTL_LEDGERS);
        }
        let configured_admin: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
        if configured_admin.as_ref() != Some(admin) {
            panic_with_error!(env, ContractError::Unauthorized);
        }
    }

    fn check_cooldown(env: &Env, resolver: &Address) -> Result<(), ContractError> {
        let cooldown: u64 = if env
            .storage()
            .persistent()
            .has(&DataKey::ResolutionCooldownSeconds)
        {
            Self::extend_ttl(
                env,
                &DataKey::ResolutionCooldownSeconds,
                COOLDOWN_TTL_LEDGERS,
            );
            env.storage()
                .persistent()
                .get(&DataKey::ResolutionCooldownSeconds)
                .unwrap_or(0)
        } else {
            0
        };
        let key = DataKey::LastResolution(resolver.clone());
        if let Some(last_resolution) = env.storage().persistent().get::<_, u64>(&key) {
            Self::extend_ttl(env, &key, COOLDOWN_TTL_LEDGERS);
            if cooldown > 0 && env.ledger().timestamp() < last_resolution.saturating_add(cooldown) {
                return Err(ContractError::ResolutionCooldownActive);
            }
        }
        Ok(())
    }
}

#[contractimpl]
impl ResolutionContract {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// Initialize the Resolution contract.
    ///
    /// Calls `version()` on `markets_contract` to verify it is a reachable,
    /// well-formed MarketsContract. The returned version is stored and
    /// re-validated on every future resolution call.
    ///
    /// # Parameters
    ///
    /// * `admin`            — Address that owns this contract. Must sign the
    ///                        transaction (`require_auth` enforced).
    /// * `markets_contract` — Address of the MarketsContract to validate and
    ///                        store. Must expose a `version() -> u32` entrypoint.
    ///
    /// # Errors
    ///
    /// * [`ContractError::InvalidState`]           — Already initialized.
    /// * [`ContractError::MarketsContractInvalid`] — Cross-contract `version()` call
    ///   failed; the address is not a valid MarketsContract.
    ///
    /// # Auth
    ///
    /// Requires a valid signature from `admin`.
    pub fn initialize(
        env: Env,
        admin: Address,
        markets_contract: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(ContractError::InvalidState);
        }

        // Validate the markets contract by calling its version() entrypoint.
        let markets_version = call_markets_version(&env, &markets_contract)?;

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::MarketsContract, &markets_contract);
        env.storage()
            .instance()
            .set(&DataKey::ExpectedMarketsVersion, &markets_version);
        env.storage().instance().set(&DataKey::Initialized, &true);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    /// Resolve a market by updating its state in the MarketsContract.
    ///
    /// Before performing any cross-contract state update, this function
    /// re-validates the stored MarketsContract address by calling its
    /// `version()` entrypoint. The live version must match the value recorded
    /// at initialization, ensuring no contract substitution has occurred since
    /// deployment.
    ///
    /// # Parameters
    ///
    /// * `admin`      — Must be the stored admin. Must sign the call.
    /// * `market_id`  — Identifier of the market to resolve.
    /// * `outcome`    — The winning outcome to record.
    ///
    /// # Errors
    ///
    /// * [`ContractError::InvalidState`]           — Not yet initialized.
    /// * [`ContractError::Unauthorized`]           — Caller is not the stored admin.
    /// * [`ContractError::MarketsContractInvalid`] — Re-validation `version()` call
    ///   failed; the stored address is no longer reachable.
    /// * [`ContractError::MarketsVersionMismatch`] — Live version differs from the
    ///   version recorded at initialization.
    ///
    /// # Auth
    ///
    /// Requires a valid signature from `admin`.
    pub fn resolve(
        env: Env,
        admin: Address,
        market_id: Symbol,
        outcome: soroban_sdk::String,
    ) -> Result<(), ContractError> {
        admin.require_auth();

        Self::assert_initialized(&env)?;
        Self::assert_is_admin(&env, &admin)?;

        let markets_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::MarketsContract)
            .ok_or(ContractError::InvalidState)?;

        let expected_version: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ExpectedMarketsVersion)
            .ok_or(ContractError::InvalidState)?;

        // Re-validate the markets contract on every resolution call.
        let live_version = call_markets_version(&env, &markets_contract)?;
        if live_version != expected_version {
            return Err(ContractError::MarketsVersionMismatch);
        }

        // Cross-contract call: update market state.
        env.invoke_contract::<()>(
            &markets_contract,
            &symbol_short!("resolve"),
            soroban_sdk::vec![&env, market_id.into_val(&env), outcome.into_val(&env)],
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read-only
    // -----------------------------------------------------------------------

    /// Return the contract version for the resolution subsystem.
    ///
    /// This read-only introspection entrypoint does not require authentication.
    pub fn version(_env: Env) -> u32 {
        7
    }

    /// Return the stored MarketsContract address, or `None` before initialization.
    pub fn markets_contract(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::MarketsContract)
    }

    /// Return the MarketsContract version recorded at initialization, or `None`
    /// before initialization.
    pub fn expected_markets_version(env: Env) -> Option<u32> {
        env.storage()
            .instance()
            .get(&DataKey::ExpectedMarketsVersion)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn assert_initialized(env: &Env) -> Result<(), ContractError> {
        if !env.storage().instance().has(&DataKey::Initialized) {
            return Err(ContractError::InvalidState);
        }
        Ok(())
    }

    fn assert_is_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::Unauthorized)?;
        if stored != *caller {
            panic_with_error!(env, ContractError::Unauthorized);
        }
        Ok(())
    }
}
