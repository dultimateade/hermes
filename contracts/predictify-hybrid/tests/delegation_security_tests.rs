//! Security and edge case tests for the delegation registry.
//!
//! This module tests security constraints, edge cases, and boundary conditions
//! of the delegation system to ensure robustness against manipulation and misuse.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

mod tests {
    use super::*;

    // ===== HELPER FUNCTIONS =====

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);
        let addr_c = Address::generate(&env);
        let addr_d = Address::generate(&env);
        (env, addr_a, addr_b, addr_c, addr_d)
    }

    // ===== SELF-DELEGATION TESTS =====

    /// Test: Self-delegation is prevented
    ///
    /// Verifies that an account cannot delegate to itself under any circumstances.
    #[test]
    fn test_self_delegation_prevented() {
        let (env, addr_a, _, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            addr_a.require_auth();

            let result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_a.clone(),
            );

            assert!(result.is_err(), "Self-delegation should fail");
        });
    }

    /// Test: Self-delegation error is consistent
    ///
    /// Verifies that self-delegation always fails with the expected error.
    #[test]
    fn test_self_delegation_error_consistency() {
        let (env, addr_a, _, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            for _ in 0..3 {
                addr_a.require_auth();

                let result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                    env.clone(),
                    addr_a.clone(),
                    addr_a.clone(),
                );

                assert!(result.is_err(), "Self-delegation should consistently fail");
            }
        });
    }

    // ===== CIRCULAR DELEGATION TESTS =====

    /// Test: Two-step circular delegation is prevented (A->B->A)
    ///
    /// Verifies that the system prevents circular chains where B cannot
    /// delegate back to A if A already delegates to B.
    #[test]
    fn test_circular_delegation_two_step() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // A -> B
            addr_a.require_auth();
            let result1 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );
            assert!(result1.is_ok(), "A->B should succeed");

            // B -> A should fail (circular)
            addr_b.require_auth();
            let result2 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_b.clone(),
                addr_a.clone(),
            );

            assert!(result2.is_err(), "B->A should fail (circular)");
        });
    }

    /// Test: Three-step circular delegation is NOT prevented
    ///
    /// Note: The current implementation only prevents direct circular delegation (A->B->A).
    /// It does not prevent longer cycles like A->B->C->A. This is a documented limitation.
    /// Preventing all cycles would require walking the full delegation graph, which is
    /// expensive on Soroban storage. For now, we test that direct cycles are blocked.
    #[test]
    fn test_three_step_cycle_note() {
        // This test documents that A->B->C->A is currently not prevented.
        // This is a trade-off between security and gas efficiency.
        // In practice, delegators should be cautious about creating delegation chains.
        let (env, addr_a, addr_b, addr_c, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // A -> B
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // B -> C (allowed, no direct cycle)
            addr_b.require_auth();
            let result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_b.clone(),
                addr_c.clone(),
            );
            assert!(result.is_ok(), "B->C is allowed (not a direct cycle)");

            // Note: C -> A would create a cycle A->B->C->A, but this is not prevented
            // by the current implementation. This is documented as a limitation.
        });
    }

    // ===== DOUBLE DELEGATION TESTS =====

    /// Test: Changing delegation replaces the old one
    ///
    /// Verifies that when a delegator establishes a new delegation, the old one
    /// is automatically replaced (each delegator has only one active delegate).
    #[test]
    fn test_double_delegation_replaces_old() {
        let (env, addr_a, addr_b, addr_c, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // A -> B
            addr_a.require_auth();
            let result1 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );
            assert!(result1.is_ok());

            // Verify A -> B exists
            assert!(predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));

            // A -> C (replaces A -> B)
            addr_a.require_auth();
            let result2 = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_c.clone(),
            );
            assert!(result2.is_ok());

            // Verify A -> C exists now
            assert!(predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_c.clone(),
            ));

            // Verify A -> B is removed from active delegate
            let active = predictify_hybrid::PredictifyHybrid::get_active_delegate(
                env.clone(),
                addr_a.clone(),
            );
            assert_eq!(active, Some(addr_c), "Active delegate should be updated");
        });
    }

    /// Test: Existing delegation to B can be revoked and re-established
    ///
    /// Verifies that revocation and re-establishment work correctly.
    #[test]
    fn test_revoke_and_restablish_delegation() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish A -> B
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Revoke
            addr_a.require_auth();
            let revoke_result = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );
            assert!(revoke_result.is_ok());

            // Verify it's gone
            assert!(!predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));

            // Re-establish A -> B
            addr_a.require_auth();
            let restablish_result = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );
            assert!(restablish_result.is_ok(), "Should be able to re-establish");

            // Verify it exists again
            assert!(predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a,
                addr_b,
            ));
        });
    }

    // ===== REVOCATION HISTORY TESTS =====

    /// Test: Revocation history is recorded on unset
    ///
    /// Verifies that when a delegation is revoked, it's recorded in the
    /// revocation history for compliance and audit purposes.
    #[test]
    fn test_revocation_history_on_unset() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Revoke
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Check history
            let history = predictify_hybrid::PredictifyHybrid::get_revocation_history(
                env.clone(),
                addr_a,
                addr_b,
            )
            .unwrap();

            assert_eq!(history.len(), 1, "Should have one revocation record");
        });
    }

    /// Test: Multiple revocations create multiple history entries
    ///
    /// Verifies that re-establishing and re-revoking a delegation creates
    /// multiple entries in the revocation history.
    #[test]
    fn test_multiple_revocations_in_history() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Cycle 1: establish and revoke
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Cycle 2: establish and revoke again
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Check history
            let history = predictify_hybrid::PredictifyHybrid::get_revocation_history(
                env.clone(),
                addr_a,
                addr_b,
            )
            .unwrap();

            assert_eq!(history.len(), 2, "Should have two revocation records");
        });
    }

    // ===== DELEGATE SELF-REVOCATION TESTS =====

    /// Test: Delegate can self-revoke their authority
    ///
    /// Verifies that a delegate can unilaterally disable their own authority
    /// without the delegator's involvement. This is useful for security incidents.
    #[test]
    fn test_delegate_self_revocation() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish A -> B
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // B self-revokes
            addr_b.require_auth();
            let result = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );
            assert!(result.is_ok(), "Delegate should be able to self-revoke");

            // Verify delegation is gone
            assert!(!predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a,
                addr_b,
            ));
        });
    }

    /// Test: Self-revocation is recorded with a marker
    ///
    /// Verifies that self-revocations (initiated by delegate) are recorded
    /// with a marker distinguishing them from delegator-initiated revocations.
    #[test]
    fn test_self_revocation_marked_in_history() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Establish delegation
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Delegate self-revokes
            addr_b.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::revoke_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Check history - should have a "self_rev" marker
            let history = predictify_hybrid::PredictifyHybrid::get_revocation_history(
                env.clone(),
                addr_a,
                addr_b,
            )
            .unwrap();

            assert_eq!(history.len(), 1);
            assert!(
                history[0].reason.is_some(),
                "Self-revocation should have a reason marker"
            );
        });
    }

    // ===== QUERY CONSISTENCY TESTS =====

    /// Test: Query consistency between has_delegation and get_active_delegate
    ///
    /// Verifies that has_delegation and get_active_delegate are consistent.
    #[test]
    fn test_query_consistency() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Initially no delegation
            assert!(!predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));
            assert_eq!(
                predictify_hybrid::PredictifyHybrid::get_active_delegate(
                    env.clone(),
                    addr_a.clone()
                ),
                None
            );

            // Establish delegation
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Both queries should reflect the delegation
            assert!(predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));
            assert_eq!(
                predictify_hybrid::PredictifyHybrid::get_active_delegate(
                    env.clone(),
                    addr_a.clone()
                ),
                Some(addr_b)
            );

            // Revoke delegation
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::unset_delegate(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            // Both queries should reflect the removal
            assert!(!predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));
            assert_eq!(
                predictify_hybrid::PredictifyHybrid::get_active_delegate(
                    env.clone(),
                    addr_a.clone()
                ),
                None
            );
        });
    }

    /// Test: get_delegations_by_delegator returns at most one delegation
    ///
    /// Verifies that since only one active delegate is allowed per delegator,
    /// get_delegations_by_delegator returns 0 or 1 elements.
    #[test]
    fn test_delegations_by_delegator_at_most_one() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // No delegation
            let result1 = predictify_hybrid::PredictifyHybrid::get_delegations_by_delegator(
                env.clone(),
                addr_a.clone(),
            )
            .unwrap();
            assert_eq!(result1.len(), 0);

            // Establish delegation
            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            let result2 = predictify_hybrid::PredictifyHybrid::get_delegations_by_delegator(
                env.clone(),
                addr_a.clone(),
            )
            .unwrap();
            assert_eq!(result2.len(), 1);
        });
    }

    // ===== EMPTY STATE TESTS =====

    /// Test: Querying non-existent delegations returns None/empty
    ///
    /// Verifies that queries for delegations that don't exist return
    /// appropriate empty results.
    #[test]
    fn test_empty_state_queries() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            // Query for non-existent delegation
            assert!(!predictify_hybrid::PredictifyHybrid::has_delegation(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            ));

            assert_eq!(
                predictify_hybrid::PredictifyHybrid::get_active_delegate(
                    env.clone(),
                    addr_a.clone()
                ),
                None
            );

            let delegations = predictify_hybrid::PredictifyHybrid::get_delegations_by_delegator(
                env.clone(),
                addr_a.clone(),
            )
            .unwrap();
            assert_eq!(delegations.len(), 0);

            let history =
                predictify_hybrid::PredictifyHybrid::get_revocation_history(env.clone(), addr_a, addr_b)
                    .unwrap();
            assert_eq!(history.len(), 0);
        });
    }

    // ===== TIMESTAMP TESTS =====

    /// Test: Delegations record creation timestamp
    ///
    /// Verifies that delegation creation records include activation timestamp
    /// for audit purposes.
    #[test]
    fn test_delegation_records_timestamp() {
        let (env, addr_a, addr_b, _, _) = setup();

        let contract_id = env.register(predictify_hybrid::PredictifyHybrid, ());

        env.as_contract(&contract_id, || {
            let before_time = env.ledger().timestamp();

            addr_a.require_auth();
            let _ = predictify_hybrid::PredictifyHybrid::delegate_votes(
                env.clone(),
                addr_a.clone(),
                addr_b.clone(),
            );

            let after_time = env.ledger().timestamp();

            let delegations = predictify_hybrid::PredictifyHybrid::get_delegations_by_delegator(
                env.clone(),
                addr_a,
            )
            .unwrap();

            assert_eq!(delegations.len(), 1);
            let delegation = &delegations[0];
            // Timestamp should be within the ledger time window
            assert!(delegation.activated_at >= before_time);
            assert!(delegation.activated_at <= after_time);
        });
    }
}
