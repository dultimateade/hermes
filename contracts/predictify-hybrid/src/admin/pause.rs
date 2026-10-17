#![allow(unused_imports)]

use super::*;

pub const LAST_ADMIN_ACTION: &str = "LAST_ADMIN_ACT";

// 2. Define the cooldown duration (24 hours = 86,400 seconds)
// For testing purposes, you might want to make this configurable,
// but b#009 usually implies a strict window.
pub const COOLDOWN_PERIOD: u64 = 86_400;

/// Internal helper to enforce a wait period between critical admin actions.
/// Call this inside any function that modifies sensitive contract state.
pub fn check_and_update_cooldown(env: &Env) {
    let now = env.ledger().timestamp();

    // Get the timestamp of the last action (default to 0 if never called)
    let last_action: u64 = env.storage().instance().get(&LAST_ADMIN_ACTION).unwrap_or(0);

    if last_action > 0 {
        // Ensure (now - last_action) >= COOLDOWN_PERIOD
        if now < last_action.checked_add(COOLDOWN_PERIOD).expect("Overflow") {
            panic_with_error!(env, AdminError::CooldownActive);
        }
    }

    // Update the timestamp to the current time for the next action
    env.storage().instance().set(&LAST_ADMIN_ACTION, &now);
}

// --- Critical Admin Functions ---

pub fn set_voting_parameters(env: Env, admin: Address, /* other params */) {
    admin.require_auth();

    // 3. ENFORCE COOLDOWN (The fix for #1099)
    check_and_update_cooldown(&env);

    // ... rest of the logic to update parameters ...
}

// ===== CONTRACT PAUSE AND ADMIN TRANSFER =====

const CONTRACT_PAUSED_KEY: &str = "ContractPaused";

/// Storage key holding the address staged by [`ContractPauseManager::transfer_admin`]
/// until it calls [`ContractPauseManager::accept_admin`].
const PENDING_ADMIN_KEY: &str = "PendingAdmin";

/// Contract-level pause and primary admin transfer.
pub struct ContractPauseManager;

impl ContractPauseManager {
    /// Returns true if the contract is currently paused.
    pub fn is_contract_paused(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&Symbol::new(env, CONTRACT_PAUSED_KEY))
            .unwrap_or(false)
    }

    /// Pause contract operations. Caller must be the current primary admin.
    pub fn pause(env: &Env, admin: &Address) -> Result<(), Error> {
        AdminAccessControl::require_admin_auth(env, admin)?;
        env.storage()
            .persistent()
            .set(&Symbol::new(env, CONTRACT_PAUSED_KEY), &true);
        EventEmitter::emit_contract_paused(env, admin);
        AuditTrailManager::append_record(
            env,
            AuditAction::ContractPaused,
            admin.clone(),
            Map::new(env),
            None,
        );
        Ok(())
    }

    /// Unpause contract operations. Caller must be the current primary admin.
    pub fn unpause(env: &Env, admin: &Address) -> Result<(), Error> {
        AdminAccessControl::require_admin_auth(env, admin)?;
        env.storage()
            .persistent()
            .set(&Symbol::new(env, CONTRACT_PAUSED_KEY), &false);
        EventEmitter::emit_contract_unpaused(env, admin);
        AuditTrailManager::append_record(
            env,
            AuditAction::ContractUnpaused,
            admin.clone(),
            Map::new(env),
            None,
        );
        Ok(())
    }

    /// Require that the contract is not paused; return Error::InvalidState otherwise.
    pub fn require_not_paused(env: &Env) -> Result<(), Error> {
        if Self::is_contract_paused(env) {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Step 1 of 2: stage a new primary admin.
    ///
    /// Caller must be the current primary admin and `new_admin` must pass
    /// [`AdminValidator::validate_admin_address`]. The role is **not** moved
    /// here: the address is only recorded as pending, and the current admin
    /// keeps full control until the pending address calls
    /// [`Self::accept_admin`]. This means a typo (or an arbitrary attacker
    /// address) can no longer permanently orphan the contract — the transfer
    /// can be corrected by calling this function again or aborted with
    /// [`Self::cancel_admin_transfer`].
    pub fn transfer_admin(
        env: &Env,
        current_admin: &Address,
        new_admin: &Address,
    ) -> Result<(), Error> {
        AdminAccessControl::require_admin_auth(env, current_admin)?;
        if new_admin == current_admin {
            return Err(Error::InvalidInput);
        }
        AdminValidator::validate_admin_address(env, new_admin)?;
        env.storage()
            .persistent()
            .set(&Symbol::new(env, PENDING_ADMIN_KEY), new_admin);
        EventEmitter::emit_admin_transfer_started(env, current_admin, new_admin);
        Ok(())
    }

    /// Step 2 of 2: complete a staged admin transfer.
    ///
    /// Must be called by the exact address staged through [`Self::transfer_admin`],
    /// which proves the new admin controls the address before it takes over.
    pub fn accept_admin(env: &Env, new_admin: &Address) -> Result<(), Error> {
        new_admin.require_auth();

        let pending: Address = Self::get_pending_admin(env).ok_or(Error::AdminNotSet)?;
        if &pending != new_admin {
            return Err(Error::Unauthorized);
        }
        AdminValidator::validate_admin_address(env, new_admin)?;

        let previous_admin = AdminAccessControl::get_admin(env)?;

        env.storage()
            .persistent()
            .set(&Symbol::new(env, "Admin"), new_admin);
        env.storage()
            .persistent()
            .remove(&Symbol::new(env, PENDING_ADMIN_KEY));

        EventEmitter::emit_admin_transferred(env, &previous_admin, new_admin);
        AuditTrailManager::append_record(
            env,
            AuditAction::AdminTransferred,
            new_admin.clone(),
            Map::new(env),
            None,
        );
        Ok(())
    }

    /// Abort an in-flight admin transfer. Caller must be the current primary admin.
    ///
    /// After this call nobody is staged and the current admin remains in place.
    pub fn cancel_admin_transfer(env: &Env, current_admin: &Address) -> Result<(), Error> {
        AdminAccessControl::require_admin_auth(env, current_admin)?;
        let pending: Address = Self::get_pending_admin(env).ok_or(Error::AdminNotSet)?;
        env.storage()
            .persistent()
            .remove(&Symbol::new(env, PENDING_ADMIN_KEY));
        EventEmitter::emit_admin_transfer_canceled(env, current_admin, &pending);
        Ok(())
    }

    /// Address staged by [`Self::transfer_admin`] and awaiting [`Self::accept_admin`], if any.
    pub fn get_pending_admin(env: &Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&Symbol::new(env, PENDING_ADMIN_KEY))
    }
}

/// State for oracle admin cooldowns
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct OracleAdminCooldownState {
    pub cooldown_seconds: u64,
    pub last_action_timestamp: u64,
}

pub struct OracleAdminCooldownManager;

impl OracleAdminCooldownManager {
    pub fn get_state(env: &Env) -> OracleAdminCooldownState {
        env.storage().persistent().get(&crate::storage::DataKey::OracleAdminCooldownState)
            .unwrap_or(OracleAdminCooldownState {
                cooldown_seconds: 0,
                last_action_timestamp: 0,
            })
    }

    pub fn set_cooldown(env: &Env, admin: &Address, cooldown_seconds: u64) -> Result<(), Error> {
        admin.require_auth();
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::ConfigAdmin)?;
        let mut state = Self::get_state(env);
        state.cooldown_seconds = cooldown_seconds;
        env.storage().persistent().set(&crate::storage::DataKey::OracleAdminCooldownState, &state);
        Ok(())
    }

    pub fn enforce_cooldown(env: &Env, admin: &Address) -> Result<(), Error> {
        let mut state = Self::get_state(env);
        if state.cooldown_seconds == 0 {
            return Ok(());
        }
        let current_time = env.ledger().timestamp();

        let cooldown_end = state.last_action_timestamp.checked_add(state.cooldown_seconds)
            .ok_or(Error::Overflow)?;

        if current_time < cooldown_end {
            EventEmitter::emit_oracle_admin_cooldown_hit(env, admin, state.last_action_timestamp, state.cooldown_seconds);
            return Err(Error::OracleAdminCooldownActive);
        }

        state.last_action_timestamp = current_time;
        env.storage().persistent().set(&crate::storage::DataKey::OracleAdminCooldownState, &state);

        Ok(())
    }
}

/// State for betting admin cooldowns
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BettingAdminCooldownState {
    pub cooldown_seconds: u64,
    pub last_action_timestamp: u64,
}

pub struct BettingAdminCooldownManager;

impl BettingAdminCooldownManager {
    pub fn get_state(env: &Env) -> BettingAdminCooldownState {
        env.storage().persistent().get(&crate::storage::DataKey::BettingAdminCooldownState)
            .unwrap_or(BettingAdminCooldownState {
                cooldown_seconds: 0,
                last_action_timestamp: 0,
            })
    }

    pub fn set_cooldown(env: &Env, admin: &Address, cooldown_seconds: u64) -> Result<(), Error> {
        admin.require_auth();
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::ConfigAdmin)?;
        let mut state = Self::get_state(env);
        state.cooldown_seconds = cooldown_seconds;
        env.storage().persistent().set(&crate::storage::DataKey::BettingAdminCooldownState, &state);
        Ok(())
    }

    pub fn enforce_cooldown(env: &Env, admin: &Address) -> Result<(), Error> {
        let mut state = Self::get_state(env);
        if state.cooldown_seconds == 0 {
            return Ok(());
        }
        let current_time = env.ledger().timestamp();

        let cooldown_end = state.last_action_timestamp.checked_add(state.cooldown_seconds)
            .ok_or(Error::Overflow)?;

        if current_time < cooldown_end {
            EventEmitter::emit_betting_admin_cooldown_hit(env, admin, state.last_action_timestamp, state.cooldown_seconds);
            return Err(Error::BettingAdminCooldownActive);
        }

        state.last_action_timestamp = current_time;
        env.storage().persistent().set(&crate::storage::DataKey::BettingAdminCooldownState, &state);

        Ok(())
    }
}

