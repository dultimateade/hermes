use disputes::{
    CouncilVote, DisputeOutcome, DisputeStatus, DisputesContract, DisputesContractClient,
    EscalationPolicy, EscalationRecord, DEFAULT_COUNCIL_THRESHOLD, DEFAULT_QUORUM,
    ESCALATION_STAKE_THRESHOLD,
};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

#[test]
fn dispute_lifecycle_is_persisted_and_uses_unique_ids() {
    let env = Env::default();
    // mock_all_auths is required because withdraw_dispute calls caller.require_auth().
    env.mock_all_auths();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);
    let market_id = Symbol::new(&env, "MARKET1");
    let creator = Address::generate(&env);

    let opened = client.open_dispute(&market_id, &creator);
    assert_eq!(opened.id, 1);
    assert_eq!(opened.status, DisputeStatus::Open);
    assert_eq!(client.get_dispute(&opened.id), Some(opened.clone()));

    // start_voting reads from storage — a forged creator in the input struct is
    // ignored because the contract always reloads the authoritative record.
    let mut stale_input = opened.clone();
    stale_input.creator = Address::generate(&env); // forged; must be ignored
    let voting = client.start_voting(&stale_input);
    assert_eq!(voting.creator, creator);
    assert_eq!(voting.status, DisputeStatus::Voting);
    assert_eq!(client.get_dispute(&opened.id), Some(voting.clone()));

    let resolved = client.resolve_dispute(&voting, &DisputeOutcome::ChallengerWins);
    assert_eq!(resolved.status, DisputeStatus::Resolved);
    assert_eq!(resolved.outcome, DisputeOutcome::ChallengerWins);
    assert_eq!(client.get_dispute(&opened.id), Some(resolved));

    let second = client.open_dispute(&Symbol::new(&env, "MARKET2"), &creator);
    assert_eq!(second.id, 2);
    // withdraw_dispute requires the caller to be the creator and to authenticate.
    let withdrawn = client.withdraw_dispute(&second, &creator);
    assert_eq!(withdrawn.status, DisputeStatus::Withdrawn);
    assert_eq!(client.get_dispute(&second.id), Some(withdrawn));
}

/// Regression test for issue #363: `withdraw_dispute` must be rejected once the
/// dispute has transitioned to `Voting` status.
#[test]
#[should_panic(expected = "cannot withdraw a dispute once voting has begun")]
fn withdraw_dispute_rejected_after_voting_starts() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);
    let market_id = Symbol::new(&env, "MKT_VOTE");
    let creator = Address::generate(&env);

    let opened = client.open_dispute(&market_id, &creator);
    let _voting = client.start_voting(&opened);

    // Must panic — dispute is now in Voting status.
    client.withdraw_dispute(&opened, &creator);
}

/// Regression test for issue #363: `withdraw_dispute` must be rejected once the
/// dispute has been resolved.
#[test]
#[should_panic(expected = "cannot withdraw a dispute once voting has begun")]
fn withdraw_dispute_rejected_after_resolution() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);
    let market_id = Symbol::new(&env, "MKT_RES");
    let creator = Address::generate(&env);

    let opened = client.open_dispute(&market_id, &creator);
    let voting = client.start_voting(&opened);
    let _resolved = client.resolve_dispute(&voting, &DisputeOutcome::ChallengerWins);

    // Must panic — dispute is now in Resolved status.
    client.withdraw_dispute(&opened, &creator);
}

/// Verify that a non-creator cannot withdraw a dispute even when it is Open.
#[test]
#[should_panic(expected = "only the dispute creator may withdraw")]
fn withdraw_dispute_rejected_for_non_creator() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);
    let market_id = Symbol::new(&env, "MKT_AUTH");
    let creator = Address::generate(&env);
    let impostor = Address::generate(&env);

    let opened = client.open_dispute(&market_id, &creator);

    // Must panic — impostor is not the dispute creator.
    client.withdraw_dispute(&opened, &impostor);
}

#[test]
#[should_panic(expected = "council member has already voted")]
fn council_vote_rejects_duplicate_voter() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);
    let voter = Address::generate(&env);
    let policy = EscalationPolicy {
        min_stake: 1,
        quorum: 10,
        council_threshold: 2,
    };
    let record = client.escalate_dispute(
        &policy,
        &Symbol::new(&env, "MARKET"),
        &1,
        &0,
        &DisputeOutcome::Escalated,
    );

    let record = client.council_vote(&policy, &record, &voter, &CouncilVote::Uphold);
    client.council_vote(&policy, &record, &voter, &CouncilVote::Uphold);
}

// ---------------------------------------------------------------------------
// Issue #364 — EscalationPolicy runtime configuration
// ---------------------------------------------------------------------------

/// `default_escalation_policy` must return the standard defaults, not a
/// hardcoded struct literal.  Verifies that DEFAULT_* constants still seed the
/// default correctly now that construction goes through `EscalationPolicy::new`.
#[test]
fn default_escalation_policy_matches_constants() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    let policy = client.default_escalation_policy();

    assert_eq!(
        policy.min_stake, ESCALATION_STAKE_THRESHOLD,
        "default min_stake must equal ESCALATION_STAKE_THRESHOLD"
    );
    assert_eq!(
        policy.quorum, DEFAULT_QUORUM,
        "default quorum must equal DEFAULT_QUORUM"
    );
    assert_eq!(
        policy.council_threshold, DEFAULT_COUNCIL_THRESHOLD,
        "default council_threshold must equal DEFAULT_COUNCIL_THRESHOLD"
    );
}

/// `new_escalation_policy` must return a policy with the supplied values when
/// all inputs are valid.
#[test]
fn new_escalation_policy_accepts_custom_min_stake() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    let custom_min_stake: u32 = 500_000;
    let custom_quorum: u32 = 5;
    let custom_threshold: u32 = 3;

    let policy = client.new_escalation_policy(&custom_min_stake, &custom_quorum, &custom_threshold);

    assert_eq!(policy.min_stake, custom_min_stake);
    assert_eq!(policy.quorum, custom_quorum);
    assert_eq!(policy.council_threshold, custom_threshold);
}

/// A custom policy with a higher `min_stake` than the default must make a
/// low-stake market ineligible for escalation.
#[test]
fn custom_min_stake_gates_eligibility() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    let high_threshold_policy =
        client.new_escalation_policy(&5_000_000u32, &DEFAULT_QUORUM, &DEFAULT_COUNCIL_THRESHOLD);

    assert!(
        !client.is_escalation_eligible(&high_threshold_policy, &1_000_000u32, &0u32),
        "stake below custom min_stake must not be eligible"
    );
    assert!(
        client.is_escalation_eligible(&high_threshold_policy, &5_000_000u32, &0u32),
        "stake at custom min_stake must be eligible"
    );
}

/// A custom policy with a lower `min_stake` than the default must accept
/// small-stake markets that the default policy would reject.
#[test]
fn custom_min_stake_lower_than_default_accepts_small_markets() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    let small_market_stake: u32 = 100;

    let default_policy = client.default_escalation_policy();
    assert!(
        !client.is_escalation_eligible(&default_policy, &small_market_stake, &0u32),
        "default policy must reject stake below ESCALATION_STAKE_THRESHOLD"
    );

    let lenient_policy =
        client.new_escalation_policy(&50u32, &DEFAULT_QUORUM, &DEFAULT_COUNCIL_THRESHOLD);
    assert!(
        client.is_escalation_eligible(&lenient_policy, &small_market_stake, &0u32),
        "custom policy with low min_stake must accept small market stake"
    );
}

/// `new_escalation_policy` must reject a zero `min_stake`.
#[test]
#[should_panic(expected = "EscalationPolicy: min_stake must be greater than zero")]
fn new_escalation_policy_rejects_zero_min_stake() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    client.new_escalation_policy(&0u32, &DEFAULT_QUORUM, &DEFAULT_COUNCIL_THRESHOLD);
}

/// `new_escalation_policy` must reject a zero `council_threshold`.
#[test]
#[should_panic(expected = "EscalationPolicy: council_threshold must be greater than zero")]
fn new_escalation_policy_rejects_zero_council_threshold() {
    let env = Env::default();
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(&env, &contract_id);

    client.new_escalation_policy(&ESCALATION_STAKE_THRESHOLD, &DEFAULT_QUORUM, &0u32);
}

// ---------------------------------------------------------------------------
// Issue #365 — CouncilVote enum
// ---------------------------------------------------------------------------

/// Helper: set up a contract client and an escalation record ready for voting.
fn setup_escalation(
    env: &Env,
    threshold: u32,
) -> (DisputesContractClient, EscalationPolicy, EscalationRecord) {
    let contract_id = env.register(DisputesContract, ());
    let client = DisputesContractClient::new(env, &contract_id);
    let policy = EscalationPolicy {
        min_stake: 1,
        quorum: 10,
        council_threshold: threshold,
    };
    let record = client.escalate_dispute(
        &policy,
        &Symbol::new(env, "MKT"),
        &1,
        &0,
        &DisputeOutcome::Escalated,
    );
    (client, policy, record)
}

/// `CouncilVote::Abstain` increments `council_votes` but neither tally.
#[test]
fn council_vote_abstain_increments_vote_count_only() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 2);
    let voter = Address::generate(&env);

    let after = client.council_vote(&policy, &record, &voter, &CouncilVote::Abstain);

    assert_eq!(after.council_votes, 1, "abstain must increment council_votes");
    assert_eq!(after.uphold_votes, 0, "abstain must not increment uphold_votes");
    assert_eq!(after.overturn_votes, 0, "abstain must not increment overturn_votes");
    assert!(!after.resolved, "single abstain must not resolve the escalation");
}

/// `CouncilVote::Uphold` increments `uphold_votes`.
#[test]
fn council_vote_uphold_increments_uphold_tally() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 2);
    let voter = Address::generate(&env);

    let after = client.council_vote(&policy, &record, &voter, &CouncilVote::Uphold);

    assert_eq!(after.uphold_votes, 1);
    assert_eq!(after.overturn_votes, 0);
    assert!(!after.resolved, "one uphold vote below threshold must not resolve");
}

/// `CouncilVote::Overturn` increments `overturn_votes`.
#[test]
fn council_vote_overturn_increments_overturn_tally() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 2);
    let voter = Address::generate(&env);

    let after = client.council_vote(&policy, &record, &voter, &CouncilVote::Overturn);

    assert_eq!(after.overturn_votes, 1);
    assert_eq!(after.uphold_votes, 0);
    assert!(!after.resolved, "one overturn vote below threshold must not resolve");
}

/// Reaching `council_threshold` uphold votes marks the escalation resolved.
#[test]
fn council_vote_uphold_resolves_at_threshold() {
    let env = Env::default();
    // Threshold of 1 so a single vote resolves immediately.
    let (client, policy, record) = setup_escalation(&env, 1);
    let voter = Address::generate(&env);

    let after = client.council_vote(&policy, &record, &voter, &CouncilVote::Uphold);

    assert!(after.resolved, "reaching threshold must set resolved = true");
}

/// Reaching `council_threshold` overturn votes marks the escalation resolved.
#[test]
fn council_vote_overturn_resolves_at_threshold() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 1);
    let voter = Address::generate(&env);

    let after = client.council_vote(&policy, &record, &voter, &CouncilVote::Overturn);

    assert!(after.resolved, "reaching overturn threshold must set resolved = true");
}

/// A duplicate vote by the same voter must be rejected regardless of variant.
#[test]
#[should_panic(expected = "council member has already voted")]
fn council_vote_duplicate_overturn_rejected() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 3);
    let voter = Address::generate(&env);

    let record = client.council_vote(&policy, &record, &voter, &CouncilVote::Overturn);
    client.council_vote(&policy, &record, &voter, &CouncilVote::Overturn);
}

/// A duplicate abstain by the same voter must also be rejected.
#[test]
#[should_panic(expected = "council member has already voted")]
fn council_vote_duplicate_abstain_rejected() {
    let env = Env::default();
    let (client, policy, record) = setup_escalation(&env, 3);
    let voter = Address::generate(&env);

    let record = client.council_vote(&policy, &record, &voter, &CouncilVote::Abstain);
    client.council_vote(&policy, &record, &voter, &CouncilVote::Abstain);
}

/// Any vote cast after the escalation is already resolved must be rejected.
#[test]
#[should_panic(expected = "escalation already resolved")]
fn council_vote_rejected_after_resolution() {
    let env = Env::default();
    // Threshold of 1 — first vote resolves immediately.
    let (client, policy, record) = setup_escalation(&env, 1);
    let first_voter = Address::generate(&env);
    let second_voter = Address::generate(&env);

    let resolved = client.council_vote(&policy, &record, &first_voter, &CouncilVote::Uphold);
    assert!(resolved.resolved);

    // Second vote must be rejected.
    client.council_vote(&policy, &resolved, &second_voter, &CouncilVote::Uphold);
}