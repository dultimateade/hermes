#![cfg(test)]

use markets::{ContractError, MarketsContract, MarketsContractClient, PayoutCurveConfig};
use soroban_sdk::{
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
    client.transfer_ownership(&admin1, &admin1);

    TestEnv {
        env,
        admin1,
        admin2,
        creator,
        bettor1,
        bettor2,
        bettor3,
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

    let proposal = te.client.get_pause_proposal(&proposal_id).unwrap();
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
    let proposal_after_app = te.client.get_pause_proposal(&proposal_id).unwrap();
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

// ===========================================================================
//  ISSUE #014: CONFIGURABLE PAYOUT CURVES TESTS
// ===========================================================================

#[test]
fn test_default_winner_takes_all_payout() {
    let te = setup_test_env();
    let m1 = create_standard_market(&te, 2);

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

    // Losing bettor gets error
    let lose_res = te.client.try_claim_winnings(&te.bettor2, &m1);
    assert_eq!(
        lose_res.unwrap_err().unwrap(),
        ContractError::InvalidOutcome.into()
    );
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
    );
    assert_eq!(
        res2.unwrap_err().unwrap(),
        ContractError::InvalidPayoutCurve.into()
    );
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
