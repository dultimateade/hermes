#![no_std]

//! Admin contract with gas-efficient entrypoints.
//!
//! Provides core admin functionality with per-entrypoint gas regression testing.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    AdminCooldownSeconds,
    AdminLastAction(Symbol),
    Admin,
    PendingOwnership,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    AlreadyInitialized = 1,
    AdminNotSet = 2,
    Unauthorized = 3,
    AdminActionTimelocked = 4,
    InvalidCooldown = 5,
    InvalidNewOwner = 6,
    OwnershipTransferNotProposed = 7,
    OwnershipTransferTimelocked = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingOwnership {
    pub new_admin: Address,
    pub proposed_at: u64,
    pub accept_after: u64,
}

const MIN_COOLDOWN_SECONDS: u64 = 60;
pub const OWNERSHIP_TRANSFER_TIMELOCK_SECONDS: u64 = 86_400;
fn get_admin_cooldown_internal(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::AdminCooldownSeconds)
        .unwrap_or(0)
}

fn check_admin_cooldown_internal(
    env: &Env,
    admin: &Address,
    function_name: &Symbol,
) -> Result<(), ContractError> {
    admin.require_auth();
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::AdminNotSet)?;
    if *admin != stored_admin {
        return Err(ContractError::Unauthorized);
    }
    let cooldown = get_admin_cooldown_internal(env);
    if cooldown == 0 {
        return Ok(());
    }
    let now = env.ledger().timestamp();
    let last_key = DataKey::AdminLastAction(function_name.clone());
    let last_action: u64 = env
        .storage()
        .persistent()
        .get(&last_key)
        .unwrap_or(0);
    if last_action > 0 && now < last_action.saturating_add(cooldown) {
        return Err(ContractError::AdminActionTimelocked);
    }
    env.storage().persistent().set(&last_key, &now);
    Ok(())
}

#[contract]
pub struct AdminContract;

#[contractimpl]
impl AdminContract {
    /// Initialize the admin contract with a primary administrator.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Get the configured admin address.
    pub fn admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)
    }

    /// Set the cooldown period (in seconds) between admin actions.
    pub fn set_admin_cooldown(env: Env, admin: Address, seconds: u64) -> Result<(), ContractError> {
        admin.require_auth();
        
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;

        if admin != stored_admin {
            return Err(ContractError::Unauthorized);
        }

        if seconds < MIN_COOLDOWN_SECONDS {
            return Err(ContractError::InvalidCooldown);
        }

        env.storage().persistent().set(&DataKey::AdminCooldownSeconds, &seconds);
        Ok(())
    }

    /// Explicitly disable admin action cooldown enforcement.
    pub fn disable_admin_cooldown(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;

        if admin != stored_admin {
            return Err(ContractError::Unauthorized);
        }

        env.storage()
            .persistent()
            .set(&DataKey::AdminCooldownSeconds, &0u64);
        Ok(())
    }

    /// Stage a new administrator for a delayed ownership transfer.
    pub fn propose_ownership(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        current_admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;

        if current_admin != stored_admin {
            return Err(ContractError::Unauthorized);
        }
        if new_admin == current_admin {
            return Err(ContractError::InvalidNewOwner);
        }

        let proposed_at = env.ledger().timestamp();
        let pending = PendingOwnership {
            new_admin,
            proposed_at,
            accept_after: proposed_at.saturating_add(OWNERSHIP_TRANSFER_TIMELOCK_SECONDS),
        };
        env.storage()
            .persistent()
            .set(&DataKey::PendingOwnership, &pending);
        Ok(())
    }

    /// Compatibility entrypoint that stages a transfer; ownership changes on accept.
    pub fn transfer_ownership(
        env: Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        Self::propose_ownership(env, current_admin, new_admin)
    }

    /// Accept a pending ownership transfer after its timelock expires.
    pub fn accept_ownership(env: Env, new_admin: Address) -> Result<(), ContractError> {
        new_admin.require_auth();

        let pending: PendingOwnership = env
            .storage()
            .persistent()
            .get(&DataKey::PendingOwnership)
            .ok_or(ContractError::OwnershipTransferNotProposed)?;

        if pending.new_admin != new_admin {
            return Err(ContractError::Unauthorized);
        }
        if env.ledger().timestamp() < pending.accept_after {
            return Err(ContractError::OwnershipTransferTimelocked);
        }

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage()
            .persistent()
            .remove(&DataKey::PendingOwnership);
        Ok(())
    }

    /// Return the staged ownership transfer, if one exists.
    pub fn pending_ownership(env: Env) -> Option<PendingOwnership> {
        env.storage()
            .persistent()
            .get(&DataKey::PendingOwnership)
    }

    /// Cancel a staged ownership transfer.
    pub fn cancel_ownership_transfer(
        env: Env,
        current_admin: Address,
    ) -> Result<(), ContractError> {
        current_admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::AdminNotSet)?;
        if current_admin != stored_admin {
            return Err(ContractError::Unauthorized);
        }
        if !env
            .storage()
            .persistent()
            .has(&DataKey::PendingOwnership)
        {
            return Err(ContractError::OwnershipTransferNotProposed);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::PendingOwnership);
        Ok(())
    }

    /// Get the configured admin cooldown period in seconds.
    pub fn get_admin_cooldown(env: Env) -> u64 {
        get_admin_cooldown_internal(&env)
    }

    /// Check and enforce admin cooldown for a specific function.
    pub fn check_admin_cooldown(env: Env, admin: Address, function_name: Symbol) -> Result<(), ContractError> {
        check_admin_cooldown_internal(&env, &admin, &function_name)
    }
}
