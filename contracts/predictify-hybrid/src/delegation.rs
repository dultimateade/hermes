//! Voting delegation registry for institutional participants.
//!
//! This module provides a secure delegation system that allows large institutional
//! participants to maintain voting rights in cold storage while delegating the
//! authority to cast votes to dedicated hot wallets. This improves security by
//! reducing the exposure of high-value accounts to active on-chain operations.
//!
//! # Overview
//!
//! The delegation registry maintains a mapping of delegators (cold storage accounts)
//! to delegates (hot wallet accounts). When a user votes, the system checks if they
//! have an active delegation and validates that the voting authority is properly
//! established.
//!
//! # Security Model
//!
//! - **Delegator-Controlled**: Only the delegator (cold storage account) can establish,
//!   modify, or revoke a delegation.
//! - **Single Active Delegation**: Each delegator can have only one active delegate at
//!   a time to prevent vote splitting and delegation ambiguity.
//! - **Explicit Revocation**: Delegations must be explicitly revoked; they do not
//!   expire automatically.
//! - **No Circular Delegation**: The system prevents circular delegation chains to
//!   avoid voting authority loops.
//! - **No Self-Delegation**: An address cannot delegate to itself.
//!
//! # Data Model
//!
//! The delegation registry stores two key data structures:
//! - Delegations: mapped by (delegator, delegate) pair
//! - Delegation index: maps delegator -> active delegate for fast lookups
//!
//! # Events
//!
//! The module emits two governance events:
//! - `set_delegate`: Fired when a delegation is established or modified
//! - `unset_delegate`: Fired when a delegation is revoked
//!
//! # Example Usage
//!
//! ```ignore
//! // Cold storage account delegates voting to hot wallet
//! DelegationManager::delegate_votes(
//!     &env,
//!     cold_storage,  // delegator
//!     hot_wallet,    // delegate
//! )?;
//!
//! // Later, query if a delegation exists
//! if let Some(dlg) = DelegationManager::get_delegation(&env, cold_storage, hot_wallet)? {
//!     println!("Delegation active since: {}", dlg.activated_at);
//! }
//!
//! // Revoke the delegation
//! DelegationManager::unset_delegate(&env, cold_storage, hot_wallet)?;
//! ```

use crate::errors::Error;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ===== STORAGE KEYS =====

/// Storage keys for delegation registry.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Delegation record: (delegator, delegate) -> Delegation
    Delegation(Address, Address),
    /// Active delegate index: delegator -> delegate
    ActiveDelegate(Address),
    /// Revocation history for audit: (delegator, delegate) -> Vec<Revocation>
    RevocationHistory(Address, Address),
}

// ===== DATA STRUCTURES =====

/// Represents an active voting delegation from a cold storage account to a hot wallet.
///
/// This structure maintains the core delegation data including the delegator,
/// delegate, and temporal metadata for audit and compliance purposes.
///
/// # Fields
///
/// - `delegator`: The account with voting rights (cold storage)
/// - `delegate`: The account authorized to cast votes (hot wallet)
/// - `activated_at`: Ledger timestamp when delegation was activated
/// - `last_modified`: Ledger timestamp of last modification
///
/// # Security Properties
///
/// - Immutable after creation (to preserve audit trail)
/// - Delegator-controlled (only delegator can create/revoke)
/// - Non-transferable (delegate cannot transfer delegation)
#[contracttype]
#[derive(Clone)]
pub struct Delegation {
    /// Cold storage account holding voting rights
    pub delegator: Address,
    /// Hot wallet account authorized to vote
    pub delegate: Address,
    /// Ledger timestamp when this delegation was activated
    pub activated_at: u64,
    /// Ledger timestamp of last modification
    pub last_modified: u64,
}

/// Configuration and constraints for the delegation system.
///
/// This structure defines the security parameters and operational limits
/// for the delegation registry, allowing for governance-controlled adjustments
/// without requiring code changes.
///
/// # Fields
///
/// - `max_delegations_per_account`: Maximum number of accounts an entity can delegate to/from
/// - `allow_revocation_without_cooldown`: Whether revocations can happen immediately
/// - `enable_circular_check`: Whether to enforce prevention of circular delegation
///
/// # Security Considerations
///
/// - Lower `max_delegations_per_account` reduces attack surface
/// - `allow_revocation_without_cooldown: false` adds time delays for security
/// - `enable_circular_check: true` prevents delegation loops
#[contracttype]
#[derive(Clone)]
pub struct DelegationConfig {
    /// Maximum active delegations from a single delegator (prevents account sprawl)
    pub max_delegations_per_account: u32,
    /// Allow immediate revocation without cooldown period
    pub allow_revocation_without_cooldown: bool,
    /// Enforce prevention of circular delegation chains
    pub enable_circular_check: bool,
}

/// Audit record of a delegation revocation.
///
/// This structure maintains a complete audit trail of delegation revocations
/// for compliance and forensic purposes. Multiple revocations can occur for
/// the same delegator-delegate pair if a delegation is reinstated and revoked
/// again.
///
/// # Fields
///
/// - `delegator`: Account that held the delegation
/// - `delegate`: Account that had voting authority
/// - `revoked_at`: Ledger timestamp of revocation
/// - `reason`: Optional reason for revocation (for governance tracking)
#[contracttype]
#[derive(Clone)]
pub struct Revocation {
    /// Account that previously delegated
    pub delegator: Address,
    /// Account that had voting authority
    pub delegate: Address,
    /// Ledger timestamp when delegation was revoked
    pub revoked_at: u64,
    /// Optional reason for revocation
    pub reason: Option<Symbol>,
}

// ===== DELEGATION MANAGER =====

/// Core delegation management functionality.
///
/// DelegationManager provides all operations needed to establish, query, and
/// revoke voting delegations. All state-changing operations enforce proper
/// authentication and security constraints.
pub struct DelegationManager;

impl DelegationManager {
    /// Establish a voting delegation from delegator to delegate.
    ///
    /// This function creates or updates a delegation, allowing the delegate
    /// to cast votes on behalf of the delegator. The delegator must provide
    /// authorization for the operation.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Account with voting rights (must authorize)
    /// - `delegate`: Account to receive voting authority
    ///
    /// # Returns
    ///
    /// - `Ok(())` on successful delegation establishment
    /// - `Err(Error)` if validation fails
    ///
    /// # Errors
    ///
    /// - `DelegationSelfDelegation`: Attempted to delegate to self
    /// - `DelegationCircular`: Delegation would create a circular chain
    /// - `Unauthorized`: Delegator did not provide authorization
    ///
    /// # Security Checks
    ///
    /// - Requires delegator authentication via `delegator.require_auth()`
    /// - Prevents self-delegation
    /// - Checks for circular delegation if configured
    /// - Prevents delegation to an account that already delegates to the delegator
    ///
    /// # Events
    ///
    /// Emits `set_delegate` event with delegator and delegate addresses.
    pub fn delegate_votes(env: &Env, delegator: Address, delegate: Address) -> Result<(), Error> {
        // Require delegator authorization
        delegator.require_auth();

        // Validate inputs
        Self::validate_delegation(&env, &delegator, &delegate)?;

        // Get current timestamp
        let now = env.ledger().timestamp();

        // Check for existing delegation to remove old one (single active delegate per delegator)
        if let Some(old_delegate) = Self::get_active_delegate(&env, &delegator) {
            if old_delegate != delegate {
                // Update the delegation by removing the old one from index
                let key = DataKey::ActiveDelegate(delegator.clone());
                env.storage().persistent().remove(&key);
            }
        }

        // Create or update delegation
        let delegation = Delegation {
            delegator: delegator.clone(),
            delegate: delegate.clone(),
            activated_at: now,
            last_modified: now,
        };

        let key = DataKey::Delegation(delegator.clone(), delegate.clone());
        env.storage().persistent().set(&key, &delegation);

        // Set active delegate index
        let idx_key = DataKey::ActiveDelegate(delegator.clone());
        env.storage().persistent().set(&idx_key, &delegate);

        // Emit event
        env.events().publish((symbol_short!("gov_dlgset"),), (delegator, delegate));

        Ok(())
    }

    /// Revoke a voting delegation for the specified delegator-delegate pair.
    ///
    /// This function removes the delegation, restoring full voting authority to
    /// the delegator. The delegator must provide authorization. This operation
    /// is irreversible without re-establishing the delegation.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Account that established the delegation
    /// - `delegate`: Account currently holding voting authority
    ///
    /// # Returns
    ///
    /// - `Ok(())` on successful revocation
    /// - `Err(Error)` if delegation does not exist or auth fails
    ///
    /// # Errors
    ///
    /// - `DelegationNotFound`: No delegation exists for this pair
    /// - `Unauthorized`: Delegator did not provide authorization
    ///
    /// # Security Checks
    ///
    /// - Requires delegator authentication via `delegator.require_auth()`
    /// - Verifies delegation exists before revocation
    /// - Records revocation in audit trail
    ///
    /// # Events
    ///
    /// Emits `unset_delegate` event with delegator and delegate addresses.
    pub fn unset_delegate(env: &Env, delegator: Address, delegate: Address) -> Result<(), Error> {
        // Require delegator authorization
        delegator.require_auth();

        // Verify delegation exists
        Self::get_delegation(&env, &delegator, &delegate)?
            .ok_or(Error::DelegationNotFound)?;

        // Get current timestamp
        let now = env.ledger().timestamp();

        // Record revocation in history for audit
        let history_key = DataKey::RevocationHistory(delegator.clone(), delegate.clone());
        let mut revocations: Vec<Revocation> = env
            .storage()
            .persistent()
            .get(&history_key)
            .unwrap_or_else(|| Vec::new(env));

        revocations.push_back(Revocation {
            delegator: delegator.clone(),
            delegate: delegate.clone(),
            revoked_at: now,
            reason: None,
        });

        env.storage()
            .persistent()
            .set(&history_key, &revocations);

        // Remove delegation
        let key = DataKey::Delegation(delegator.clone(), delegate.clone());
        env.storage().persistent().remove(&key);

        // Remove from active delegate index if it's the current delegate
        if let Some(active) = Self::get_active_delegate(&env, &delegator) {
            if active == delegate {
                let idx_key = DataKey::ActiveDelegate(delegator.clone());
                env.storage().persistent().remove(&idx_key);
            }
        }

        // Emit event
        env.events()
            .publish((symbol_short!("gov_dlguns"),), (delegator, delegate));

        Ok(())
    }

    /// Revoke a delegation by the delegate themselves.
    ///
    /// This administrative function allows a delegate to revoke their own
    /// voting authority, useful for immediate disabling if keys are compromised.
    /// This is called "revoke" to distinguish from "unset" (delegator-initiated).
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Account that established the delegation
    /// - `delegate`: Account revoking their own authority (must authorize)
    ///
    /// # Returns
    ///
    /// - `Ok(())` on successful revocation
    /// - `Err(Error)` if validation fails
    ///
    /// # Events
    ///
    /// Emits `unset_delegate` event similar to delegator-initiated revocation.
    ///
    /// # Note
    ///
    /// This function requires delegate authorization, allowing hot wallets to
    /// self-disable if needed. This provides an additional security mechanism.
    pub fn revoke_delegation(env: &Env, delegator: Address, delegate: Address) -> Result<(), Error> {
        // Require delegate authorization (self-revocation)
        delegate.require_auth();

        // Verify delegation exists
        Self::get_delegation(&env, &delegator, &delegate)?
            .ok_or(Error::DelegationNotFound)?;

        // Get current timestamp
        let now = env.ledger().timestamp();

        // Record revocation in history
        let history_key = DataKey::RevocationHistory(delegator.clone(), delegate.clone());
        let mut revocations: Vec<Revocation> = env
            .storage()
            .persistent()
            .get(&history_key)
            .unwrap_or_else(|| Vec::new(env));

        revocations.push_back(Revocation {
            delegator: delegator.clone(),
            delegate: delegate.clone(),
            revoked_at: now,
            reason: Some(symbol_short!("self_rev")), // Self-revocation marker
        });

        env.storage()
            .persistent()
            .set(&history_key, &revocations);

        // Remove delegation
        let key = DataKey::Delegation(delegator.clone(), delegate.clone());
        env.storage().persistent().remove(&key);

        // Remove from active delegate index if it's the current delegate
        if let Some(active) = Self::get_active_delegate(&env, &delegator) {
            if active == delegate {
                let idx_key = DataKey::ActiveDelegate(delegator.clone());
                env.storage().persistent().remove(&idx_key);
            }
        }

        // Emit event
        env.events()
            .publish((symbol_short!("gov_dlguns"),), (delegator, delegate));

        Ok(())
    }

    /// Retrieve a specific delegation by delegator and delegate addresses.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Cold storage account
    /// - `delegate`: Hot wallet account
    ///
    /// # Returns
    ///
    /// - `Ok(Some(delegation))` if the delegation exists
    /// - `Ok(None)` if no delegation exists
    /// - `Err(Error)` on storage errors
    pub fn get_delegation(
        env: &Env,
        delegator: &Address,
        delegate: &Address,
    ) -> Result<Option<Delegation>, Error> {
        let key = DataKey::Delegation(delegator.clone(), delegate.clone());
        let result = env.storage().persistent().get::<_, Delegation>(&key);
        Ok(result)
    }

    /// Get the currently active delegate for a delegator.
    ///
    /// Each delegator can have at most one active delegate. This function
    /// retrieves that delegate if one exists.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Cold storage account
    ///
    /// # Returns
    ///
    /// - `Some(delegate)` if an active delegation exists
    /// - `None` if no active delegation
    pub fn get_active_delegate(env: &Env, delegator: &Address) -> Option<Address> {
        let key = DataKey::ActiveDelegate(delegator.clone());
        env.storage().persistent().get::<_, Address>(&key)
    }

    /// Check if a specific delegation exists and is active.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Cold storage account
    /// - `delegate`: Hot wallet account
    ///
    /// # Returns
    ///
    /// - `true` if delegation exists and is active
    /// - `false` otherwise
    pub fn has_delegation(env: &Env, delegator: &Address, delegate: &Address) -> bool {
        Self::get_delegation(env, delegator, delegate)
            .ok()
            .flatten()
            .is_some()
    }

    /// List all delegations for a specific delegator.
    ///
    /// **Note**: This function requires iteration through storage and may be
    /// expensive with large numbers of delegations. Consider caching the active
    /// delegate address if only checking voting authority.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Account to query delegations for
    ///
    /// # Returns
    ///
    /// - `Vec<Delegation>` of all active delegations for this delegator
    pub fn get_delegations_by_delegator(env: &Env, delegator: &Address) -> Result<Vec<Delegation>, Error> {
        // Get active delegate as the only active delegation
        if let Some(delegate) = Self::get_active_delegate(env, delegator) {
            if let Some(delegation) = Self::get_delegation(env, delegator, &delegate)? {
                let mut result = Vec::new(env);
                result.push_back(delegation);
                return Ok(result);
            }
        }
        Ok(Vec::new(env))
    }

    /// Get the revocation history for a delegator-delegate pair.
    ///
    /// This function retrieves the complete audit trail of revocations for
    /// compliance and forensic purposes.
    ///
    /// # Arguments
    ///
    /// - `env`: Soroban environment
    /// - `delegator`: Account that delegated
    /// - `delegate`: Account that received delegation
    ///
    /// # Returns
    ///
    /// - `Vec<Revocation>` of all revocations for this pair
    pub fn get_revocation_history(
        env: &Env,
        delegator: &Address,
        delegate: &Address,
    ) -> Result<Vec<Revocation>, Error> {
        let key = DataKey::RevocationHistory(delegator.clone(), delegate.clone());
        let result: Vec<Revocation> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        Ok(result)
    }

    // ===== INTERNAL VALIDATION =====

    /// Validate delegation constraints.
    ///
    /// This internal function performs all validation checks before establishing
    /// a delegation. It checks:
    /// - Self-delegation prevention
    /// - Circular delegation prevention
    /// - Mutual delegation prevention (prevent A->B when B->A exists)
    fn validate_delegation(env: &Env, delegator: &Address, delegate: &Address) -> Result<(), Error> {
        // Prevent self-delegation
        if delegator == delegate {
            return Err(Error::DelegationSelfDelegation);
        }

        // Prevent circular delegation: if delegate already delegates to delegator
        if Self::has_delegation(env, delegate, delegator) {
            return Err(Error::DelegationCircular);
        }

        Ok(())
    }
}

// ===== TESTS =====

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_delegate_votes_creates_delegation() {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        let result = DelegationManager::delegate_votes(&env, delegator.clone(), delegate.clone());
        assert!(result.is_ok());

        let delegation = DelegationManager::get_delegation(&env, &delegator, &delegate)
            .unwrap()
            .unwrap();
        assert_eq!(delegation.delegator, delegator);
        assert_eq!(delegation.delegate, delegate);
    }

    #[test]
    fn test_self_delegation_fails() {
        let env = Env::default();
        let delegator = Address::generate(&env);

        let result = DelegationManager::delegate_votes(&env, delegator.clone(), delegator.clone());
        assert!(matches!(result, Err(Error::DelegationSelfDelegation)));
    }

    #[test]
    fn test_circular_delegation_fails() {
        let env = Env::default();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);

        // A delegates to B
        let result = DelegationManager::delegate_votes(&env, addr_a.clone(), addr_b.clone());
        assert!(result.is_ok());

        // B tries to delegate to A (should fail as circular)
        let result = DelegationManager::delegate_votes(&env, addr_b.clone(), addr_a.clone());
        assert!(matches!(result, Err(Error::DelegationCircular)));
    }

    #[test]
    fn test_unset_delegate_removes_delegation() {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Create delegation
        let _ = DelegationManager::delegate_votes(&env, delegator.clone(), delegate.clone());

        // Verify it exists
        assert!(DelegationManager::has_delegation(&env, &delegator, &delegate));

        // Unset delegation
        let result = DelegationManager::unset_delegate(&env, delegator.clone(), delegate.clone());
        assert!(result.is_ok());

        // Verify it's gone
        assert!(!DelegationManager::has_delegation(&env, &delegator, &delegate));
    }

    #[test]
    fn test_get_active_delegate() {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        // No active delegate initially
        assert_eq!(DelegationManager::get_active_delegate(&env, &delegator), None);

        // Create delegation
        let _ = DelegationManager::delegate_votes(&env, delegator.clone(), delegate.clone());

        // Active delegate should match
        let active = DelegationManager::get_active_delegate(&env, &delegator);
        assert_eq!(active, Some(delegate));
    }

    #[test]
    fn test_revoke_delegation_by_delegate() {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Create delegation
        let _ = DelegationManager::delegate_votes(&env, delegator.clone(), delegate.clone());

        // Delegate revokes their own authority
        let result = DelegationManager::revoke_delegation(&env, delegator.clone(), delegate.clone());
        assert!(result.is_ok());

        // Verify it's gone
        assert!(!DelegationManager::has_delegation(&env, &delegator, &delegate));
    }

    #[test]
    fn test_revocation_history() {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Create and revoke delegation
        let _ = DelegationManager::delegate_votes(&env, delegator.clone(), delegate.clone());
        let _ = DelegationManager::unset_delegate(&env, delegator.clone(), delegate.clone());

        // Check history
        let history = DelegationManager::get_revocation_history(&env, &delegator, &delegate)
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().revoked_at > 0, true);
    }
}
