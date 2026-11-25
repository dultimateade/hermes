#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Map, Symbol, Vec,
};

mod migrate;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Version,
}

/// Errors returned by migration entrypoints.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Contract not initialized
    NotInitialized = 2,
    /// Contract already initialized
    AlreadyInitialized = 3,
    /// Version mismatch - current version doesn't match expected
    VersionMismatch = 4,
    /// Target version is invalid (same as current or lower)
    InvalidTargetVersion = 5,
    /// Migration data validation failed
    InvalidMigrationData = 6,
    /// Storage migration failed
    MigrationFailed = 7,
}

/// Migration metadata stored per version upgrade
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationRecord {
    pub from_version: u32,
    pub to_version: u32,
    pub timestamp: u64,
    pub migrated_by: Address,
    pub data_checksum: BytesN<32>,
}

/// Storage layout version 1 - initial schema
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageV1 {
    pub admin: Address,
    pub version: u32,
    pub data: Map<Symbol, Vec<u8>>,
}

/// Storage layout version 2 - added migration history
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageV2 {
    pub admin: Address,
    pub version: u32,
    pub data: Map<Symbol, Vec<u8>>,
    pub migration_history: Vec<MigrationRecord>,
}

/// Current storage version
const CURRENT_VERSION: u32 = 2;

#[contract]
pub struct MigrateContract;

#[contractimpl]
impl MigrateContract {
    /// Initialize the contract with admin and version
    /// Requires auth from admin
    pub fn initialize(env: Env, admin: Address, version: u32) -> Result<(), ContractError> {
        admin.require_auth();
        
        if version == 0 || version > CURRENT_VERSION {
            return Err(ContractError::InvalidTargetVersion);
        }
        
        if env.storage().instance().has(&DataKey::Version) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Version, &version);
        Ok(())
    }

    /// Returns the configured migration administrator.
    pub fn admin(env: Env) -> Result<Address, ContractError> {
        Self::load_admin(&env)
    }

    /// Returns the current stored version.
    pub fn current_version(env: Env) -> Result<u32, ContractError> {
        Self::load_version(&env)
    }

    fn load_admin(env: &Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    /// Migrate error-related state from one version to the next.
    ///
    /// This entrypoint delegates to [`migrate::migrate_error_state`] and
    /// provides the same pre-condition guarantees — version compare-and-set,
    /// admin authentication, and an extensible data-reshape hook. The stored
    /// version advances only after every migration step succeeds.
    ///
    /// # Errors
    ///
    /// See [`migrate::migrate_error_state`].
    pub fn migrate_error_data(
        env: Env,
        admin: Address,
        expected_version: u32,
        target_version: u32,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        migrate::migrate_error_state(&env, &admin, expected_version, target_version)
    }

    fn load_version(env: &Env) -> Result<u32, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Version)
            .ok_or(ContractError::NotInitialized)
    }
}
