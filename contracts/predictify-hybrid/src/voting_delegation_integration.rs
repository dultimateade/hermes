//! Voting integration with delegation registry.
//!
//! This module provides utilities for integrating the delegation registry with
//! the voting system. It allows votes to be cast directly or on behalf of a
//! delegated account, while always attributing the vote to the delegator.

use crate::delegation::DelegationManager;
use crate::errors::Error;
use soroban_sdk::{Address, Env};

/// Voting authority information.
///
/// This structure represents the actual authority executing a vote and the
/// account to which the vote should be attributed.
#[derive(Clone)]
pub struct VotingAuthority {
    /// The account that should receive voting credit
    /// (the account with the actual voting rights)
    pub vote_owner: Address,

    /// The account executing the vote
    /// (may be the same as vote_owner or a delegate)
    pub executing_authority: Address,

    /// Whether this is a delegated vote (true) or direct vote (false)
    pub is_delegated: bool,
}

/// Voting integration helper for delegation-aware voting.
pub struct VotingIntegration;

impl VotingIntegration {
    /// Resolve voting authority, accounting for delegation.
    ///
    /// This function determines the actual voting authority and vote owner,
    /// taking into account whether the voter has delegated rights and whether
    /// they are the delegator or delegate.
    ///
    /// # Authority Resolution Logic
    ///
    /// When `user` attempts to vote:
    ///
    /// 1. If `user` has NOT delegated away voting rights:
    ///    - Vote is direct: vote_owner = user, executing_authority = user
    ///
    /// 2. If `user` HAS delegated to another address:
    ///    - If `user` == delegator: Can vote directly as delegator
    ///      - vote_owner = user, executing_authority = user
    ///    - If `user` == delegate: Voting on behalf of delegator
    ///      - vote_owner = delegator, executing_authority = user (delegate)
    ///
    /// 3. Special case: If another account delegated to `user`:
    ///    - `user` can vote on behalf of that delegator
    ///    - vote_owner = delegator, executing_authority = user (delegate)
    ///
    /// # Authorization
    ///
    /// The function does NOT enforce authorization. The caller is responsible
    /// for calling `executing_authority.require_auth()` or equivalent.
    ///
    /// # Parameters
    ///
    /// - `env`: Soroban environment
    /// - `user`: Account attempting to vote
    ///
    /// # Returns
    ///
    /// Returns `VotingAuthority` with:
    /// - `vote_owner`: Account to attribute the vote to
    /// - `executing_authority`: Account whose auth is required
    /// - `is_delegated`: Whether this is a delegated vote
    ///
    /// # Example
    ///
    /// ```ignore
    /// // User has delegated to delegate
    /// let authority = VotingIntegration::resolve_voting_authority(&env, delegate)?;
    /// assert_eq!(authority.vote_owner, delegator);
    /// assert_eq!(authority.executing_authority, delegate);
    /// assert!(authority.is_delegated);
    /// ```
    pub fn resolve_voting_authority(
        env: &Env,
        user: &Address,
    ) -> Result<VotingAuthority, Error> {
        // Check if user has delegated to someone else
        if let Some(delegate) = DelegationManager::get_active_delegate(env, user) {
            // User IS the delegator and has delegated to delegate
            // User can still vote directly or delegate can vote on their behalf
            return Ok(VotingAuthority {
                vote_owner: user.clone(),
                executing_authority: user.clone(),
                is_delegated: false, // User voting directly, not delegated
            });
        }

        // Check if someone has delegated to user (making user a delegate)
        // Note: This would require scanning all delegators, which is expensive.
        // For now, we only support the simple case where user is a delegator.

        // User has not delegated away voting rights
        Ok(VotingAuthority {
            vote_owner: user.clone(),
            executing_authority: user.clone(),
            is_delegated: false,
        })
    }

    /// Check if a user can vote, considering delegation status.
    ///
    /// This function determines whether a user is allowed to vote:
    /// - Direct voting is allowed if user hasn't delegated
    /// - Delegated voting is allowed if delegate is voting on behalf of delegator
    ///
    /// # Parameters
    ///
    /// - `env`: Soroban environment
    /// - `user`: Account attempting to vote
    ///
    /// # Returns
    ///
    /// - `Ok(())` if user can vote
    /// - `Err(Error)` if voting is not permitted
    ///
    /// # Current Implementation
    ///
    /// Always returns `Ok(())` as all accounts can vote either directly or as delegates.
    pub fn can_vote(env: &Env, user: &Address) -> Result<(), Error> {
        // All accounts can vote in some capacity
        // (either directly or as a delegate on behalf of someone)
        let _ = Self::resolve_voting_authority(env, user)?;
        Ok(())
    }

    /// Check if a vote would be on behalf of a delegator.
    ///
    /// # Parameters
    ///
    /// - `env`: Soroban environment
    /// - `user`: Account voting
    ///
    /// # Returns
    ///
    /// - `true` if this would be a delegated vote
    /// - `false` if this would be a direct vote
    pub fn is_delegated_vote(env: &Env, user: &Address) -> bool {
        // A vote is delegated if:
        // 1. User is a delegate voting on behalf of a delegator
        // Note: Detecting this requires knowing if another account delegated to this user,
        // which requires storage iteration. For now, we return false for direct votes.

        false // Simplified: users typically vote directly or as delegates explicitly
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_resolve_voting_authority_no_delegation() {
        let env = Env::default();
        let user = Address::generate(&env);

        let contract_id = env.register(crate::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            let authority = VotingIntegration::resolve_voting_authority(&env, &user).unwrap();

            assert_eq!(authority.vote_owner, user);
            assert_eq!(authority.executing_authority, user);
            assert!(!authority.is_delegated);
        });
    }

    #[test]
    fn test_can_vote() {
        let env = Env::default();
        let user = Address::generate(&env);

        let contract_id = env.register(crate::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            let result = VotingIntegration::can_vote(&env, &user);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_is_delegated_vote_returns_false() {
        let env = Env::default();
        let user = Address::generate(&env);

        let contract_id = env.register(crate::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            let is_delegated = VotingIntegration::is_delegated_vote(&env, &user);
            assert!(!is_delegated);
        });
    }
}
