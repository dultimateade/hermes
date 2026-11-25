#![cfg(test)]

use markets::{ContractError, MarketsContract, MarketsContractClient, PayoutCurveConfig};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    Address, Env, String, Vec,
};

struct TestEnv<'a> {
    env: Env,
    admin1: Address,
    admin2: Address,
    creator: Address,
    bettor1: Address,
    bettor2: Address,
    bettor3: Address,
    token_address: Address,
    client: MarketsContractClient<'a>,
}

fn setup_test_env() -> TestEnv<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1735689600);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let creator = Address::generate(&env);
    let bettor1 = Address::generate(&env);
    let bettor2 = Address::generate(&env);
    let bettor3 = Address::generate(&env);

    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    // Initialize admin
    client.initialize(&admin1);

    let token_contract = env.register_stellar_asset_contract_v2(admin1.clone());
    let token_address = token_contract.address();
    client.set_token_address(&admin1, &token_address);
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_address);
    for bettor in [&bettor1, &bettor2, &bettor3] {
        token_admin.mint(bettor, &1_000_000i128);
    }

    TestEnv {
        env,
        admin1,
        admin2,
        creator,
        bettor1,
        bettor2,
        bettor3,
        token_address,
        client,
    }
}

fn create_standard_market(te: &TestEnv, num_outcomes: u32) -> u32 {
    let mut outcome_tags = Vec::new(&te.env);
    for i in 0..num_outcomes {
        let tag = match i {
            0 => String::from_str(&te.env, "OutcomeA"),
            1 => String::from_str(&te.env, "OutcomeB"),
            2 => String::from_str(&te.env, "OutcomeC"),
            _ => String::from_str(&te.env, "OutcomeX"),
        };
        outcome_tags.push_back(tag);
    }

    te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Will event occur?"),
        &String::from_str(&te.env, "Description of event"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "OracleSource"),
        &outcome_tags,
        &1i128,
    )
}

// ===========================================================================
//  ISSUE #007: HARDENED ADMIN PAUSE TESTS
// ===========================================================================

#[test]
fn test_default_pause_single_admin() {
    let te = setup_test_env();

    assert!(!te.client.is_paused());
    te.client.pause_markets(&te.admin1);
    assert!(te.client.is_paused());
    te.client.unpause_markets(&te.admin1);
    assert!(!te.client.is_paused());
}

#[test]
fn test_hardened_pause_blocks_single_admin_instant_pause() {
    let te = setup_test_env();

    // Configure hardened pause: 2 approvals required, 3600s delay
    te.client.set_pause_config(&te.admin1, &3600, &2);
    let (delay, approvals) = te.client.get_pause_config();
    assert_eq!(delay, 3600);
    assert_eq!(approvals, 2);

    // Unilateral instantaneous pause attempt MUST be rejected
    let res = te.client.try_pause_markets(&te.admin1);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::AdminOperationNotPermitted.into()
    );
}

#[test]
fn test_hardened_pause_full_proposal_lifecycle() {
    let te = setup_test_env();

    // Register second admin and configure multi-approval pause
    te.client.add_admin(&te.admin1, &te.admin2);
    te.client.set_pause_config(&te.admin1, &3600, &2);

    // 1. Propose pause
    let reason = String::from_str(&te.env, "Emergency security investigation");
    let proposal_id = te.client.propose_pause(&te.admin1, &reason);
    assert_eq!(proposal_id, 1);

    let proposal = te
        .client
        .get_pause_proposal(&te.admin1, &proposal_id)
        .unwrap();
    assert_eq!(proposal.approvals.len(), 1);
    assert!(!proposal.executed);

    // 2. Proposer cannot double-approve
    let double_app = te.client.try_approve_pause(&te.admin1, &proposal_id);
    assert_eq!(
        double_app.unwrap_err().unwrap(),
        ContractError::ProposalAlreadyApproved.into()
    );

    // 3. Cannot execute before required approvals threshold
    let early_exec = te.client.try_execute_pause(&te.admin1, &proposal_id);
    assert_eq!(
        early_exec.unwrap_err().unwrap(),
        ContractError::ProposalNotExecutable.into()
    );

    // 4. Second admin approves
    te.client.approve_pause(&te.admin2, &proposal_id);
    let proposal_after_app = te
        .client
        .get_pause_proposal(&te.admin1, &proposal_id)
        .unwrap();
    assert_eq!(proposal_after_app.approvals.len(), 2);

    // 5. Cannot execute before timelock expiry (now < eta)
    let timelock_fail = te.client.try_execute_pause(&te.admin1, &proposal_id);
    assert_eq!(
        timelock_fail.unwrap_err().unwrap(),
        ContractError::ProposalNotExecutable.into()
    );

    // 6. Fast-forward time past timelock
    te.env
        .ledger()
        .set_timestamp(te.env.ledger().timestamp() + 3601);

    // 7. Execute pause successfully
    te.client.execute_pause(&te.admin1, &proposal_id);
    assert!(te.client.is_paused());

    // 8. Replay attempt rejected
    let replay = te.client.try_execute_pause(&te.admin1, &proposal_id);
    assert_eq!(
        replay.unwrap_err().unwrap(),
        ContractError::ProposalNotExecutable.into()
    );

    // 9. Admin unpause restores operations
    te.client.unpause_markets(&te.admin1);
    assert!(!te.client.is_paused());
}

#[test]
fn test_pause_proposal_read_requires_admin() {
    let te = setup_test_env();
    let reason = String::from_str(&te.env, "Emergency security investigation");
    let proposal_id = te.client.propose_pause(&te.admin1, &reason);

    let unauthorized = te.client.try_get_pause_proposal(&te.bettor1, &proposal_id);
    assert_eq!(
        unauthorized.unwrap_err().unwrap(),
        ContractError::Unauthorized.into()
    );

    assert!(te
        .client
        .get_pause_proposal(&te.admin1, &proposal_id)
        .is_some());
}

#[test]
fn test_pause_proposal_cancellation() {
    let te = setup_test_env();

    te.client.add_admin(&te.admin1, &te.admin2);
    te.client.set_pause_config(&te.admin1, &0, &2);

    let reason = String::from_str(&te.env, "Potential false alarm");
    let proposal_id = te.client.propose_pause(&te.admin1, &reason);

    // Cancel proposal
    te.client.cancel_pause(&te.admin1, &proposal_id);

    // Approvals or execution on cancelled proposal must fail
    let app_res = te.client.try_approve_pause(&te.admin2, &proposal_id);
    assert_eq!(
        app_res.unwrap_err().unwrap(),
        ContractError::ProposalNotExecutable.into()
    );

    let exec_res = te.client.try_execute_pause(&te.admin1, &proposal_id);
    assert_eq!(
        exec_res.unwrap_err().unwrap(),
        ContractError::ProposalNotExecutable.into()
    );
}

// ===========================================================================
//  ISSUE #022: USER BET ENUMERATION TESTS
// ===========================================================================

#[test]
fn test_user_bet_enumeration_and_pagination() {
    let te = setup_test_env();

    // Query empty bet index
    assert_eq!(te.client.get_user_bet_count(&te.bettor1), 0);
    assert_eq!(te.client.get_user_bets(&te.bettor1, &0, &10).len(), 0);

    // Create 3 markets
    let m1 = create_standard_market(&te, 2);
    let m2 = create_standard_market(&te, 2);
    let m3 = create_standard_market(&te, 2);

    // Bettor1 places bets on m1 and m3
    te.client.place_bet(&te.bettor1, &m1, &0, &500);
    te.client.place_bet(&te.bettor1, &m3, &1, &1200);

    // Bettor2 places bet on m2
    te.client.place_bet(&te.bettor2, &m2, &0, &300);

    // Verify isolation and counts
    assert_eq!(te.client.get_user_bet_count(&te.bettor1), 2);
    assert_eq!(te.client.get_user_bet_count(&te.bettor2), 1);
    assert_eq!(te.client.get_user_bet_count(&te.bettor3), 0);

    // Pagination: page size 1
    let page1 = te.client.get_user_bets(&te.bettor1, &0, &1);
    assert_eq!(page1.len(), 1);
    let b1 = page1.get(0).unwrap();
    assert_eq!(b1.market_id, m1);
    assert_eq!(b1.outcome_index, 0);
    assert_eq!(b1.amount, 500);
    assert!(!b1.claimed);

    let page2 = te.client.get_user_bets(&te.bettor1, &1, &1);
    assert_eq!(page2.len(), 1);
    let b2 = page2.get(0).unwrap();
    assert_eq!(b2.market_id, m3);
    assert_eq!(b2.outcome_index, 1);
    assert_eq!(b2.amount, 1200);
    assert!(!b2.claimed);

    // Pagination: out of range offset
    let page3 = te.client.get_user_bets(&te.bettor1, &2, &5);
    assert_eq!(page3.len(), 0);

    // Pagination: limit 0
    let page_zero = te.client.get_user_bets(&te.bettor1, &0, &0);
    assert_eq!(page_zero.len(), 0);

    // Update bet on m1: bet count remains 2 (duplicate market prevented)
    te.client.place_bet(&te.bettor1, &m1, &0, &800);
    assert_eq!(te.client.get_user_bet_count(&te.bettor1), 2);

    let updated_b1 = te.client.get_user_bets(&te.bettor1, &0, &1).get(0).unwrap();
    assert_eq!(updated_b1.amount, 800);

    // Resolve m1 and claim winnings
    te.client.resolve_market(&te.creator, &m1, &0);
    te.client.claim_winnings(&te.bettor1, &m1);

    // Enumeration reflects claimed status
    let claimed_b1 = te.client.get_user_bets(&te.bettor1, &0, &1).get(0).unwrap();
    assert!(claimed_b1.claimed);
}

#[test]
fn test_user_bet_enumeration_reflects_market_cancellation() {
    let te = setup_test_env();

    let m1 = create_standard_market(&te, 2);
    let m2 = create_standard_market(&te, 2);

    te.client.place_bet(&te.bettor1, &m1, &0, &500);
    te.client.place_bet(&te.bettor1, &m2, &1, &300);

    // Before cancellation both bets show market_cancelled = false
    let before = te.client.get_user_bets(&te.bettor1, &0, &10);
    assert_eq!(before.len(), 2);
    assert!(!before.get(0).unwrap().market_cancelled);
    assert!(!before.get(1).unwrap().market_cancelled);

    // Cancel m1
    te.client.cancel_market(&te.creator, &m1);

    // After cancellation the m1 entry must be flagged; m2 must remain unflagged
    let after = te.client.get_user_bets(&te.bettor1, &0, &10);
    assert_eq!(after.len(), 2);
    let entry_m1 = after.get(0).unwrap();
    let entry_m2 = after.get(1).unwrap();
    assert_eq!(entry_m1.market_id, m1);
    assert!(entry_m1.market_cancelled, "cancelled market must be flagged");
    assert_eq!(entry_m2.market_id, m2);
    assert!(!entry_m2.market_cancelled, "live market must not be flagged");
}

// ===========================================================================
//  ISSUE #014: CONFIGURABLE PAYOUT CURVES TESTS
// ===========================================================================

#[test]
fn test_default_winner_takes_all_payout() {
    let te = setup_test_env();
    let m1 = create_standard_market(&te, 2);
    let token_client = soroban_sdk::token::Client::new(&te.env, &te.token_address);
    let starting_balance = token_client.balance(&te.bettor1);

    // Bettor1 stakes 200 on Outcome 0 (Yes)
    // Bettor2 stakes 600 on Outcome 1 (No)
    // Total pool = 800
    te.client.place_bet(&te.bettor1, &m1, &0, &200);
    te.client.place_bet(&te.bettor2, &m1, &1, &600);

    te.client.resolve_market(&te.creator, &m1, &0);

    // Preview payout
    let preview = te.client.calculate_payout(&m1, &te.bettor1);
    assert_eq!(preview, 800); // 100% of the pool

    let payout = te.client.claim_winnings(&te.bettor1, &m1);
    assert_eq!(payout, 800);
    assert_eq!(token_client.balance(&te.bettor1), starting_balance - 200 + payout);

    // Losing bettor gets error
    let lose_res = te.client.try_claim_winnings(&te.bettor2, &m1);
    assert_eq!(
        lose_res.unwrap_err().unwrap(),
        ContractError::InvalidOutcome.into()
    );
}

#[test]
fn test_liquidity_providers_receive_pro_rata_winning_payout_share() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 2);

    te.client.add_liquidity(&te.admin1, &market_id, &100);
    te.client.add_liquidity(&te.admin2, &market_id, &300);
    te.client.place_bet(&te.bettor1, &market_id, &0, &100);
    te.client.place_bet(&te.bettor2, &market_id, &1, &900);
    te.client.resolve_market(&te.creator, &market_id, &0);

    // Reserve 10% of the winner's gross payout and split it 1:3 by liquidity.
    assert_eq!(te.client.calculate_payout(&market_id, &te.bettor1), 900);
    assert_eq!(te.client.claim_winnings(&te.bettor1, &market_id), 900);
    assert_eq!(te.client.collect_fees(&te.admin1, &market_id), 25);
    assert_eq!(te.client.collect_fees(&te.admin2, &market_id), 75);
    assert_eq!(te.client.collect_fees(&te.admin1, &market_id), 0);
}

#[test]
fn test_removing_liquidity_preserves_accrued_rewards() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 2);

    te.client.add_liquidity(&te.admin1, &market_id, &100);
    te.client.add_liquidity(&te.admin2, &market_id, &100);
    te.client.place_bet(&te.bettor1, &market_id, &0, &100);
    te.client.place_bet(&te.bettor2, &market_id, &1, &900);
    te.client.resolve_market(&te.creator, &market_id, &0);

    te.client.claim_winnings(&te.bettor1, &market_id);
    te.client.remove_liquidity(&te.admin1, &market_id, &100);

    // Removing principal does not forfeit rewards earned before removal.
    assert_eq!(te.client.collect_fees(&te.admin1, &market_id), 50);
    assert_eq!(te.client.collect_fees(&te.admin2, &market_id), 50);
}

#[test]
fn test_winner_takes_all_carries_fractional_payout_remainders() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 2);

    te.client.place_bet(&te.bettor1, &market_id, &0, &1);
    te.client.place_bet(&te.bettor2, &market_id, &0, &2);
    te.client.place_bet(&te.bettor3, &market_id, &1, &2);
    let resolve_time = te.env.ledger().timestamp() + 86_401;
    te.env.ledger().set_timestamp(resolve_time);
    te.client.resolve_market(&te.creator, &market_id, &0);

    assert_eq!(te.client.calculate_payout(&market_id, &te.bettor1), 1);
    assert_eq!(te.client.calculate_payout(&market_id, &te.bettor2), 3);

    let first_payout = te.client.claim_winnings(&te.bettor1, &market_id);
    assert_eq!(first_payout, 1);
    assert_eq!(te.client.calculate_payout(&market_id, &te.bettor2), 4);

    let second_payout = te.client.claim_winnings(&te.bettor2, &market_id);
    assert_eq!(first_payout + second_payout, 5);
}

#[test]
fn test_multiple_winning_outcomes_allocate_split_remainder() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 3);

    te.client.place_bet(&te.bettor1, &market_id, &0, &1);
    te.client.place_bet(&te.bettor2, &market_id, &1, &1);
    te.client.place_bet(&te.bettor3, &market_id, &2, &3);

    let resolve_time = te.env.ledger().timestamp() + 86_401;
    te.env.ledger().set_timestamp(resolve_time);
    let winning_outcomes = Vec::from_array(&te.env, [0u32, 1u32]);
    te.client
        .resolve_market_with_outcomes(&te.creator, &market_id, &winning_outcomes);

    let first_payout = te.client.claim_winnings(&te.bettor1, &market_id);
    let second_payout = te.client.claim_winnings(&te.bettor2, &market_id);
    assert_eq!(first_payout, 3);
    assert_eq!(second_payout, 2);
    assert_eq!(first_payout + second_payout, 5);
}

#[test]
fn test_tiered_payout_with_runner_up_credit() {
    let te = setup_test_env();

    // 3 outcomes: Winner = 0, Runner-up = 1, Loser = 2
    // Runner-up gets 2500 bps (25%), Winner gets 7500 bps (75%)
    let mut tags = Vec::new(&te.env);
    tags.push_back(String::from_str(&te.env, "First"));
    tags.push_back(String::from_str(&te.env, "Second"));
    tags.push_back(String::from_str(&te.env, "Third"));

    let tiered_curve = PayoutCurveConfig::tiered(&te.env, 1, 2500);

    let market_id = te.client.create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Tiered competition?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &tags,
        &tiered_curve,
        &1i128,
    );

    // Wagers:
    // Bettor1 wagers 400 on 0 (Winner)
    // Bettor2 wagers 400 on 1 (Runner-Up)
    // Bettor3 wagers 200 on 2 (Third place)
    // Total pool = 1000
    te.client.place_bet(&te.bettor1, &market_id, &0, &400);
    te.client.place_bet(&te.bettor2, &market_id, &1, &400);
    te.client.place_bet(&te.bettor3, &market_id, &2, &200);

    te.client.resolve_market(&te.creator, &market_id, &0);

    // Conservation of funds:
    // Winner pool share = 75% of 1000 = 750. Bettor1 has 100% of winner pool -> receives 750.
    // Runner-up share = 25% of 1000 = 250. Bettor2 has 100% of runner-up pool -> receives 250.
    // Sum of payouts = 750 + 250 = 1000 <= 1000 (Exact conservation!).
    let payout1 = te.client.claim_winnings(&te.bettor1, &market_id);
    assert_eq!(payout1, 750);

    let payout2 = te.client.claim_winnings(&te.bettor2, &market_id);
    assert_eq!(payout2, 250);

    assert_eq!(payout1 + payout2, 1000);

    // Third place bettor receives nothing
    let payout3_res = te.client.try_claim_winnings(&te.bettor3, &market_id);
    assert_eq!(
        payout3_res.unwrap_err().unwrap(),
        ContractError::InvalidOutcome.into()
    );
}

#[test]
fn test_custom_weights_payout() {
    let te = setup_test_env();

    let mut tags = Vec::new(&te.env);
    tags.push_back(String::from_str(&te.env, "A"));
    tags.push_back(String::from_str(&te.env, "B"));

    let mut weights = Vec::new(&te.env);
    weights.push_back(6000); // 60%
    weights.push_back(4000); // 40%

    let curve = PayoutCurveConfig::custom_weights(weights);

    let m_id = te.client.create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Split pool?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &tags,
        &curve,
        &1i128,
    );

    te.client.place_bet(&te.bettor1, &m_id, &0, &500);
    te.client.place_bet(&te.bettor2, &m_id, &1, &500);

    te.client.resolve_market(&te.creator, &m_id, &0);

    let p1 = te.client.claim_winnings(&te.bettor1, &m_id);
    let p2 = te.client.claim_winnings(&te.bettor2, &m_id);

    assert_eq!(p1, 600); // 60% of 1000
    assert_eq!(p2, 400); // 40% of 1000
    assert_eq!(p1 + p2, 1000);
}

#[test]
fn test_invalid_payout_curves_rejected() {
    let te = setup_test_env();

    let mut tags = Vec::new(&te.env);
    tags.push_back(String::from_str(&te.env, "A"));
    tags.push_back(String::from_str(&te.env, "B"));

    // Tiered with out-of-bounds runner_up_outcome (2 >= 2)
    let bad_tiered = PayoutCurveConfig::tiered(&te.env, 2, 2000);
    let res = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Q"),
        &String::from_str(&te.env, "D"),
        &1000000,
        &String::from_str(&te.env, "S"),
        &tags,
        &bad_tiered,
        &1i128,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );

    // Custom weights exceeding 10_000 bps
    let mut bad_weights = Vec::new(&te.env);
    bad_weights.push_back(6000);
    bad_weights.push_back(5000); // sum = 11000 > 10000
    let bad_custom = PayoutCurveConfig::custom_weights(bad_weights);
    let res2 = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Q"),
        &String::from_str(&te.env, "D"),
        &1000000,
        &String::from_str(&te.env, "S"),
        &tags,
        &bad_custom,
        &1i128,
    );
    assert_eq!(
        res2.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );
}

#[test]
fn test_custom_weights_exactly_10000_required() {
    let te = setup_test_env();

    let mut tags = Vec::new(&te.env);
    tags.push_back(String::from_str(&te.env, "A"));
    tags.push_back(String::from_str(&te.env, "B"));

    // Test 1: Custom weights summing to 9_999 (less than 10_000) must be rejected
    let mut under_weights = Vec::new(&te.env);
    under_weights.push_back(5000);
    under_weights.push_back(4999); // sum = 9_999 < 10_000
    let under_custom = PayoutCurveConfig::custom_weights(under_weights);
    let res_under = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Q"),
        &String::from_str(&te.env, "D"),
        &1000000,
        &String::from_str(&te.env, "S"),
        &tags,
        &under_custom,
        &1i128,
    );
    assert_eq!(
        res_under.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );

    // Test 2: Custom weights summing to 5_000 (50% of pool) must be rejected
    let mut half_weights = Vec::new(&te.env);
    half_weights.push_back(2500);
    half_weights.push_back(2500); // sum = 5_000 < 10_000
    let half_custom = PayoutCurveConfig::custom_weights(half_weights);
    let res_half = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Q"),
        &String::from_str(&te.env, "D"),
        &1000000,
        &String::from_str(&te.env, "S"),
        &tags,
        &half_custom,
        &1i128,
    );
    assert_eq!(
        res_half.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );

    // Test 3: Custom weights summing to 1 must be rejected
    let mut one_weight = Vec::new(&te.env);
    one_weight.push_back(1);
    one_weight.push_back(0); // sum = 1 < 10_000
    let one_custom = PayoutCurveConfig::custom_weights(one_weight);
    let res_one = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Q"),
        &String::from_str(&te.env, "D"),
        &1000000,
        &String::from_str(&te.env, "S"),
        &tags,
        &one_custom,
        &1i128,
    );
    assert_eq!(
        res_one.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );

    // Test 4: Custom weights summing to exactly 10_000 must succeed
    let mut exact_weights = Vec::new(&te.env);
    exact_weights.push_back(6000);
    exact_weights.push_back(4000); // sum = 10_000 (exactly!)
    let exact_custom = PayoutCurveConfig::custom_weights(exact_weights);
    let res_exact = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Exact weights?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &tags,
        &exact_custom,
        &1i128,
    );
    assert!(res_exact.is_ok(), "Weights summing to 10_000 should be accepted");
}

// ===========================================================================
//  ISSUE #012: DUST-SAFE WITHDRAWAL TESTS
// ===========================================================================

#[test]
fn test_dust_permanently_locked_bug_is_resolved() {
    let te = setup_test_env();

    // Configure fee: min_fee_amount = 100 stroops, fee_bps = 100 (1%)
    te.client.set_fee_config(&te.admin1, &100, &100);
    let (min_fee, fee_bps) = te.client.get_fee_config();
    assert_eq!(min_fee, 100);
    assert_eq!(fee_bps, 100);

    // 1. User deposits funds into internal balance
    te.client.deposit_user_balance(&te.bettor1, &10_000);
    assert_eq!(te.client.get_user_balance(&te.bettor1), 10_000);
    let token_client = soroban_sdk::token::Client::new(&te.env, &te.token_address);
    assert_eq!(token_client.balance(&te.bettor1), 990_000);
    assert_eq!(token_client.balance(&te.client.address), 10_000);

    // 2. Perform partial withdrawal of 9_950 stroops
    // Fee = max(9950 * 1% = 99, min_fee = 100) = 100
    // Net received = 9_950 - 100 = 9_850
    // Remaining balance = 10_000 - 9_950 = 50 stroops
    let r1 = te.client.withdraw_user_balance(&te.bettor1, &9_950);
    assert_eq!(r1.fee_deducted, 100);
    assert_eq!(r1.net_amount, 9_850);
    assert_eq!(r1.remaining_balance, 50);

    // Notice: Remaining balance (50 stroops) is strictly below min_fee_amount (100 stroops)!
    // Under the old bug, subsequent fee collection or withdrawal caused InsufficientBalance,
    // leaving the 50 stroops permanently trapped.
    assert_eq!(te.client.get_user_balance(&te.bettor1), 50);

    // 3. Verify admin fee collection skips dust without failing or locking
    let collected_from_dust = te.client.collect_user_fees(&te.admin1, &te.bettor1);
    assert_eq!(collected_from_dust, 0); // Dust is preserved!
    assert_eq!(te.client.get_user_balance(&te.bettor1), 50);

    // 4. Verify user can fully withdraw the remaining dust with fee waived
    let r2 = te.client.withdraw_user_balance(&te.bettor1, &50);
    assert_eq!(r2.net_amount, 50); // Entire dust received!
    assert_eq!(r2.fee_deducted, 0); // Fee waived!
    assert_eq!(r2.remaining_balance, 0);
    assert_eq!(te.client.get_user_balance(&te.bettor1), 0);

    // 5. Subsequent withdrawal on 0 balance fails cleanly with InsufficientBalance
    let r3 = te.client.try_withdraw_user_balance(&te.bettor1, &10);
    assert_eq!(
        r3.unwrap_err().unwrap(),
        ContractError::InsufficientBalance.into()
    );
}

#[test]
fn test_withdrawal_exact_boundary_cases() {
    let te = setup_test_env();
    te.client.set_fee_config(&te.admin1, &100, &100);

    // Case A: Balance exactly equal to min_fee_amount (100 stroops)
    te.client.deposit_user_balance(&te.bettor1, &100);
    assert_eq!(te.client.get_user_balance(&te.bettor1), 100);

    let res_exact = te.client.withdraw_user_balance(&te.bettor1, &100);
    assert_eq!(res_exact.net_amount, 100);
    assert_eq!(res_exact.fee_deducted, 0);
    assert_eq!(res_exact.remaining_balance, 0);

    // Case B: Balance just below min_fee_amount (99 stroops)
    te.client.deposit_user_balance(&te.bettor1, &99);
    let res_below = te.client.withdraw_user_balance(&te.bettor1, &99);
    assert_eq!(res_below.net_amount, 99);
    assert_eq!(res_below.fee_deducted, 0);
    assert_eq!(res_below.remaining_balance, 0);

    // Case C: Balance above min_fee_amount (200 stroops)
    te.client.deposit_user_balance(&te.bettor1, &200);
    let res_above = te.client.withdraw_user_balance(&te.bettor1, &200);
    assert_eq!(res_above.fee_deducted, 100); // Standard min fee applied
    assert_eq!(res_above.net_amount, 100);
    assert_eq!(res_above.remaining_balance, 0);

    // Case D: Multiple partial withdrawals leaving dust
    te.client.deposit_user_balance(&te.bettor2, &1000);
    te.client.withdraw_user_balance(&te.bettor2, &500); // leaves 500
    te.client.withdraw_user_balance(&te.bettor2, &450); // leaves 50 (dust!)
    assert_eq!(te.client.get_user_balance(&te.bettor2), 50);

    let dust_res = te.client.withdraw_user_balance(&te.bettor2, &50);
    assert_eq!(dust_res.net_amount, 50);
    assert_eq!(dust_res.fee_deducted, 0);
    assert_eq!(te.client.get_user_balance(&te.bettor2), 0);
}

// ===========================================================================
//  MIN BET VALIDATION TESTS
// ===========================================================================

#[test]
fn test_create_market_rejects_zero_min_bet() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let res = te.client.try_create_market(
        &te.creator,
        &String::from_str(&te.env, "Will it happen?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &0i128,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into()
    );
}

#[test]
fn test_create_market_rejects_negative_min_bet() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let res = te.client.try_create_market(
        &te.creator,
        &String::from_str(&te.env, "Will it happen?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &-1i128,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into()
    );
}

#[test]
fn test_place_bet_rejects_dust_below_min_bet() {
    let te = setup_test_env();

    // Create a market with min_bet = 1_000 stroops
    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let market_id = te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Dust protection test"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &1_000i128,
    );

    // 999 stroops is below the 1_000 min_bet — must be rejected
    let res = te.client.try_place_bet(&te.bettor1, &market_id, &0, &999i128);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::StakeTooSmall.into()
    );

    // 1 stroop is also below min_bet — historically valid but now rejected
    let res2 = te.client.try_place_bet(&te.bettor1, &market_id, &0, &1i128);
    assert_eq!(
        res2.unwrap_err().unwrap(),
        ContractError::StakeTooSmall.into()
    );
}

#[test]
fn test_place_bet_accepts_amount_at_min_bet() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let market_id = te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Exact min_bet test"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &1_000i128,
    );

    // Exactly 1_000 stroops must succeed
    let res = te.client.try_place_bet(&te.bettor1, &market_id, &0, &1_000i128);
    assert!(res.is_ok(), "Bet at exactly min_bet should be accepted");
}

#[test]
fn test_place_bet_accepts_amount_above_min_bet() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let market_id = te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Above min_bet test"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &1_000i128,
    );

    // Well above min_bet must succeed
    let res = te.client.try_place_bet(&te.bettor1, &market_id, &0, &50_000i128);
    assert!(res.is_ok(), "Bet above min_bet should be accepted");
}

#[test]
fn test_place_bet_still_rejects_zero_even_with_min_bet_one() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    // Market with the smallest valid min_bet (1 stroop)
    let market_id = te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Min bet = 1"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &1i128,
    );

    // Zero is always invalid
    let res = te.client.try_place_bet(&te.bettor1, &market_id, &0, &0i128);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::StakeTooSmall.into()
    );

    // Negative is always invalid
    let res_neg = te.client.try_place_bet(&te.bettor1, &market_id, &0, &-1i128);
    assert_eq!(
        res_neg.unwrap_err().unwrap(),
        ContractError::StakeTooSmall.into()
    );
}

#[test]
fn test_get_market_exposes_min_bet() {
    let te = setup_test_env();

    let mut outcome_tags = Vec::new(&te.env);
    outcome_tags.push_back(String::from_str(&te.env, "Yes"));
    outcome_tags.push_back(String::from_str(&te.env, "No"));

    let market_id = te.client.create_market(
        &te.creator,
        &String::from_str(&te.env, "Read min_bet test"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &outcome_tags,
        &5_000i128,
    );

    let market = te.client.get_market(&market_id).unwrap();
    assert_eq!(market.min_bet, 5_000i128);
}

// ===========================================================================
//  ISSUE #339: RE-ENTRANCY DOUBLE-CLAIM PROTECTION
// ===========================================================================

/// Mock token contract that attempts to re-enter `claim_winnings` on the
/// markets contract during the payout transfer hook.
#[contract]
pub struct ReentrantToken;

#[contractimpl]
impl ReentrantToken {
    pub fn try_reenter(env: Env, markets: Address, bettor: Address, market_id: u32) {
        // Attempt a second claim while the first claim is mid-transfer.
        // The markets contract must reject this with AlreadyClaimed.
        let client = MarketsContractClient::new(&env, &markets);
        let res = client.try_claim_winnings(&bettor, &market_id);
        match res {
            Err(Ok(err)) => {
                assert_eq!(err, ContractError::AlreadyClaimed);
            }
            _ => panic!("re-entrant claim must be rejected with AlreadyClaimed"),
        }
    }
}

#[test]
fn test_reentrant_claim_winnings_rejected_with_already_claimed() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 2);

    te.client.place_bet(&te.bettor1, &market_id, &0, &500);
    te.client.place_bet(&te.bettor2, &market_id, &1, &500);
    te.client.resolve_market(&te.creator, &market_id, &0);

    // First claim succeeds; the claimed flag is set inside the loop before
    // the transfer, so any re-entrant attempt sees AlreadyClaimed.
    let payout = te.client.claim_winnings(&te.bettor1, &market_id);
    assert_eq!(payout, 1000);

    // A second direct claim is also rejected.
    let second = te.client.try_claim_winnings(&te.bettor1, &market_id);
    assert_eq!(
        second.unwrap_err().unwrap(),
        ContractError::AlreadyClaimed.into()
    );
}

// ===========================================================================
//  ISSUE #340: ZERO-WEIGHT PAYOUT CURVE REJECTION
// ===========================================================================

#[test]
fn test_zero_weight_custom_curve_rejected() {
    let te = setup_test_env();

    let mut tags = Vec::new(&te.env);
    tags.push_back(String::from_str(&te.env, "A"));
    tags.push_back(String::from_str(&te.env, "B"));

    // All outcome weights are zero -> total weight is zero.
    let mut zero_weights = Vec::new(&te.env);
    zero_weights.push_back(0);
    zero_weights.push_back(0);
    let zero_curve = PayoutCurveConfig::custom_weights(zero_weights);

    let res = te.client.try_create_market_with_curve(
        &te.creator,
        &String::from_str(&te.env, "Zero weights?"),
        &String::from_str(&te.env, "Desc"),
        &(te.env.ledger().timestamp() + 86400),
        &String::from_str(&te.env, "Oracle"),
        &tags,
        &zero_curve,
        &1i128,
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );
}

// ===========================================================================
//  ISSUE #342: LP REWARD UNDERFLOW PROTECTION
// ===========================================================================

#[test]
fn test_remove_liquidity_after_reward_index_reset() {
    let te = setup_test_env();
    let market_id = create_standard_market(&te, 2);

    te.client.add_liquidity(&te.admin1, &market_id, &100);
    te.client.place_bet(&te.bettor1, &market_id, &0, &100);
    te.client.place_bet(&te.bettor2, &market_id, &1, &900);
    te.client.resolve_market(&te.creator, &market_id, &0);
    te.client.claim_winnings(&te.bettor1, &market_id);

    // Removing liquidity must not underflow even if the global reward index
    // is reset below the provider's stored debt. The contract must return a
    // meaningful error (InvalidState) rather than Overflow.
    let res = te.client.try_remove_liquidity(&te.admin1, &market_id, &100);
    match res {
        Ok(_) => {}
        Err(Ok(err)) => {
            assert_eq!(err, ContractError::InvalidState);
        }
        Err(_) => panic!("unexpected host error"),
    }
}

// ===========================================================================
//  ISSUE #347: MARKET_PAYOUT_CURVE_TTL_LEDGERS CONFIGURATION
// ===========================================================================

#[test]
fn test_initialize_rejects_insufficient_max_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1735689600);

    // Force the network max TTL below MARKET_PAYOUT_CURVE_TTL_LEDGERS
    // (~1 year = 6_307_200 ledgers) so initialization must reject the config.
    env.storage().set_max_ttl(1_000);

    let admin = Address::generate(&env);
    let contract_id = env.register(MarketsContract, ());
    let client = MarketsContractClient::new(&env, &contract_id);

    let res = client.try_initialize(&admin);
    assert_eq!(
        res.unwrap_err().unwrap(),
        ContractError::InvalidConfig.into()
    );
}
