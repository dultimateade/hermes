//! Delegation-aware voting helpers.
//!
//! This module keeps delegation-specific voting logic separated from the core
//! voting mechanics defined in [`crate::voting`]. It re-exports the shared
//! integration implementation so both the historical path and the dedicated
//! delegation module remain available.

pub use crate::voting_delegation_integration::{VotingAuthority, VotingIntegration};

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_voting_integration_is_available_from_delegation_module() {
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
}
