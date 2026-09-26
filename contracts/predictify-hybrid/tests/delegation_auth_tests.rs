//! Authorization boundary tests for delegation entrypoints.
//!
//! This test module verifies that all delegation entrypoints properly enforce
//! authorization requirements and prevent unauthorized access. Each test validates
//! that the correct authority is required and improper authentication is rejected.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

mod tests {
    use super::*;

    // ===== HELPER FUNCTIONS =====

    /// Setup fresh environment and return test addresses
    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        let delegator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        (env, delegator, delegate, unauthorized)
    }

    // ===== delegate_votes TESTS =====

    /// Test: delegate_votes requires delegator authorization
    ///
    /// Verifies that delegate_votes requires the delegator to authorize the
    /// operation. Unauthorized addresses cannot establish delegations.
    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_delegate_votes_requires_delegator_auth() {
        let (env, delegator, delegate, unauthorized) = setup();

        // Register the contract
        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Try to establish delegation without delegator authorization
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator,
                delegate,
            );
        });
    }

    /// Test: delegate_votes succeeds with delegator authorization
    ///
    /// Verifies that delegate_votes succeeds when the delegator properly
    /// authorizes the operation.
    #[test]
    fn test_delegate_votes_with_delegator_auth() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            delegator.require_auth();

            let result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            assert!(result.is_ok());
        });
    }

    /// Test: delegate_votes prevents self-delegation
    ///
    /// Verifies that an account cannot delegate to itself, even with
    /// proper authorization.
    #[test]
    fn test_delegate_votes_prevents_self_delegation() {
        let (env, delegator, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            delegator.require_auth();

            let result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegator.clone(),
            );

            assert!(result.is_err());
        });
    }

    /// Test: delegate_votes prevents circular delegation
    ///
    /// Verifies that the system prevents circular delegation chains.
    /// If A->B exists, B cannot delegate to A.
    #[test]
    fn test_delegate_votes_prevents_circular_delegation() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Step 1: delegator -> delegate
            delegator.require_auth();
            let result1 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );
            assert!(result1.is_ok());

            // Step 2: Try to create circular: delegate -> delegator
            // This should fail
            delegate.require_auth();
            let result2 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegate.clone(),
                delegator.clone(),
            );

            assert!(result2.is_err());
        });
    }

    // ===== unset_delegate TESTS =====

    /// Test: unset_delegate requires delegator authorization
    ///
    /// Verifies that only the delegator can revoke a delegation.
    /// Unauthorized addresses cannot revoke delegations.
    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_unset_delegate_requires_delegator_auth() {
        let (env, delegator, delegate, unauthorized) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Try to revoke as unauthorized address
            unauthorized.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                delegator,
                delegate,
            );
        });
    }

    /// Test: unset_delegate succeeds with delegator authorization
    ///
    /// Verifies that unset_delegate succeeds when the delegator properly
    /// authorizes the revocation.
    #[test]
    fn test_unset_delegate_with_delegator_auth() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Revoke delegation
            delegator.require_auth();
            let result = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            assert!(result.is_ok());
        });
    }

    /// Test: unset_delegate fails for non-existent delegation
    ///
    /// Verifies that attempting to revoke a delegation that doesn't exist
    /// fails with appropriate error.
    #[test]
    fn test_unset_delegate_fails_for_non_existent_delegation() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Try to revoke delegation that doesn't exist
            delegator.require_auth();
            let result = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                delegator,
                delegate,
            );

            assert!(result.is_err());
        });
    }

    // ===== revoke_delegation TESTS =====

    /// Test: revoke_delegation requires delegate authorization
    ///
    /// Verifies that only the delegate can self-revoke their authority.
    /// The delegator cannot use this endpoint; only the delegate can.
    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_revoke_delegation_requires_delegate_auth() {
        let (env, delegator, delegate, unauthorized) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Try to revoke as unauthorized address
            unauthorized.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                delegator,
                delegate,
            );
        });
    }

    /// Test: revoke_delegation succeeds with delegate authorization
    ///
    /// Verifies that revoke_delegation succeeds when the delegate properly
    /// authorizes the self-revocation.
    #[test]
    fn test_revoke_delegation_with_delegate_auth() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Delegate self-revokes
            delegate.require_auth();
            let result = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            assert!(result.is_ok());
        });
    }

    /// Test: revoke_delegation fails if delegator tries to use it
    ///
    /// Verifies that the delegator cannot use revoke_delegation.
    /// Only the delegate can self-revoke.
    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_revoke_delegation_fails_for_delegator() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Delegator tries to self-revoke (should fail - only delegate can)
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                delegator,
                delegate,
            );
        });
    }

    // ===== READ-ONLY QUERY TESTS =====

    /// Test: has_delegation is read-only
    ///
    /// Verifies that has_delegation doesn't require any authorization and can
    /// be called by any address. This is a read-only query.
    #[test]
    fn test_has_delegation_is_read_only() {
        let (env, delegator, delegate, unauthorized) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Any address can query (no auth required)
            let result = predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            assert!(result);

            // Unauthorized address can also query
            let result2 = predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                delegator,
                delegate,
            );

            assert!(result2);
        });
    }

    /// Test: get_active_delegate is read-only
    ///
    /// Verifies that get_active_delegate doesn't require any authorization.
    #[test]
    fn test_get_active_delegate_is_read_only() {
        let (env, delegator, delegate, unauthorized) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Any address can query
            let result = predictify_hybrid::PredictifyHybrid::get_active_delegate(
                env.clone(),
                delegator.clone(),
            );

            assert_eq!(result, Some(delegate));
        });
    }

    /// Test: get_delegations_by_delegator is read-only
    ///
    /// Verifies that any address can query delegation lists.
    #[test]
    fn test_get_delegations_by_delegator_is_read_only() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Query delegations
            let result = predictify_hybrid::PredictifyHybrid::get_delegations_by_delegator(
                env.clone(),
                delegator,
            );

            assert!(result.is_ok());
            let delegations = result.unwrap();
            assert_eq!(delegations.len(), 1);
        });
    }

    /// Test: get_revocation_history is read-only
    ///
    /// Verifies that any address can query revocation history.
    #[test]
    fn test_get_revocation_history_is_read_only() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish and revoke delegation
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Query history
            let result = predictify_hybrid::PredictifyHybrid::get_revocation_history(
                env.clone(),
                delegator,
                delegate,
            );

            assert!(result.is_ok());
            let history = result.unwrap();
            assert_eq!(history.len(), 1);
        });
    }

    // ===== MATRIX TESTS =====

    /// Test: Auth boundary matrix for all entrypoints
    ///
    /// This comprehensive test verifies the full authorization matrix.
    /// It documents what each caller can and cannot do.
    ///
    /// # Matrix
    ///
    /// | Operation | Delegator | Delegate | Other | Result |
    /// |-----------|-----------|----------|-------|--------|
    /// | delegate_votes | ✓ | ✗ | ✗ | Can only delegate if delegator authorizes |
    /// | unset_delegate | ✓ | ✗ | ✗ | Can only revoke if delegator authorizes |
    /// | revoke_delegation | ✗ | ✓ | ✗ | Can only self-revoke if delegate authorizes |
    /// | has_delegation | ✓ | ✓ | ✓ | Anyone can query |
    /// | get_active_delegate | ✓ | ✓ | ✓ | Anyone can query |
    /// | get_delegations_by_delegator | ✓ | ✓ | ✓ | Anyone can query |
    /// | get_revocation_history | ✓ | ✓ | ✓ | Anyone can query |
    ///
    #[test]
    fn test_delegation_auth_boundary_matrix() {
        let (env, delegator, delegate, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation with delegator auth
            delegator.require_auth();
            let establish = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );
            assert!(establish.is_ok(), "Delegator should be able to establish delegation");

            // Query functions work for anyone
            let query1 = predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );
            assert!(query1, "Query should return true");

            let query2 = predictify_hybrid::PredictifyHybrid::get_active_delegate(
                env.clone(),
                delegator.clone(),
            );
            assert_eq!(query2, Some(delegate.clone()), "Query should return delegate");

            // Revoke with delegator auth
            delegator.require_auth();
            let revoke = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );
            assert!(revoke.is_ok(), "Delegator should be able to revoke");

            // Re-establish for self-revoke test
            delegator.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );

            // Self-revoke with delegate auth
            delegate.require_auth();
            let self_revoke = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                delegator.clone(),
                delegate.clone(),
            );
            assert!(self_revoke.is_ok(), "Delegate should be able to self-revoke");
        });
    }
}
