// contracts/reporting/tests/event_emission.rs
//
// Integration tests verifying event emission for the reporting lifecycle.
//
// `contracts/reporting/src/events.rs` currently defines only the pause/unpause
// events (`reporting_paused` and `reporting_unpaused`). The lifecycle
// entrypoints (submit_report, verify_report, dispute_report, resolve_dispute,
// update_report_status, delete_report) do not currently emit any events. These
// tests document that behaviour and assert that no unexpected events are
// emitted. If events are added to those entrypoints in the future, the
// corresponding assertions below should be updated to check the expected event.

mod common;

use common::{dispute_reason, register_and_init, report_data, report_hash};
use reporting::{ReportingError};
use soroban_sdk::{testutils::Events, Symbol, TryFromVal, Val};

/// Helper: assert that `events` contains exactly one event matching the given
/// topics and data, and return it.
fn assert_single_event(
    env: &soroban_sdk::Env,
    expected_topic0: Symbol,
    expected_topic1: soroban_sdk::Address,
    expected_data: (),
) {
    let events = env.events().all();
    assert_eq!(events.len(), 1, "expected exactly one event");

    let (contract, topics, data) = events.get(0).unwrap();

    // The emitting contract should be the reporting contract.
    let _ = contract;

    assert_eq!(topics.len(), 2, "expected two topics");
    let topic0 = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
    let topic1 = soroban_sdk::Address::try_from_val(env, &topics.get(1).unwrap()).unwrap();
    assert_eq!(topic0, expected_topic0);
    assert_eq!(topic1, expected_topic1);

    let decoded: () = <()>::try_from_val(env, &data).unwrap();
    assert_eq!(decoded, expected_data);
}

#[test]
fn test_pause_emits_event() {
    let (env, client, admin) = register_and_init();

    client.pause_reporting(&admin);

    assert_single_event(
        &env,
        Symbol::new(&env, "reporting_paused"),
        admin.clone(),
        (),
    );
}

#[test]
fn test_pause_is_idempotent_no_new_event() {
    let (env, client, admin) = register_and_init();

    client.pause_reporting(&admin);
    let after_first = env.events().all();
    assert_eq!(after_first.len(), 1, "first pause should emit one event");

    // Second pause should be a no-op and emit no new event.
    client.pause_reporting(&admin);
    let after_second = env.events().all();
    assert_eq!(
        after_second.len(),
        after_first.len(),
        "second pause should not emit a new event"
    );
    assert_eq!(after_second, after_first);
}

#[test]
fn test_unpause_emits_event() {
    let (env, client, admin) = register_and_init();

    client.pause_reporting(&admin);
    // Reset event log by taking a snapshot; we only care about the unpause event.
    let before_unpause = env.events().all();

    client.unpause_reporting(&admin);
    let after_unpause = env.events().all();

    assert_eq!(
        after_unpause.len(),
        before_unpause.len() + 1,
        "unpause should emit exactly one new event"
    );

    let (_, topics, data) = after_unpause.get(after_unpause.len() - 1).unwrap();
    assert_eq!(topics.len(), 2);
    let topic0 = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
    let topic1 = soroban_sdk::Address::try_from_val(&env, &topics.get(1).unwrap()).unwrap();
    assert_eq!(topic0, Symbol::new(&env, "reporting_unpaused"));
    assert_eq!(topic1, admin);
    let decoded: () = <()>::try_from_val(&env, &data).unwrap();
    assert_eq!(decoded, ());
}

#[test]
fn test_unpause_is_idempotent_no_new_event() {
    let (env, client, admin) = register_and_init();

    client.pause_reporting(&admin);
    client.unpause_reporting(&admin);
    let after_first_unpause = env.events().all();

    // Second unpause should be a no-op and emit no new event.
    client.unpause_reporting(&admin);
    let after_second_unpause = env.events().all();
    assert_eq!(
        after_second_unpause.len(),
        after_first_unpause.len(),
        "second unpause should not emit a new event"
    );
    assert_eq!(after_second_unpause, after_first_unpause);
}

#[test]
fn test_lifecycle_entrypoints_emit_no_unexpected_events() {
    let (env, client, admin) = register_and_init();
    let reporter = common::reporter(&env);
    let data = report_data(&env);
    let hash = report_hash(&env);

    // submit_report
    let before = env.events().all();
    let report_id = client.submit_report(&reporter, &data, &hash);
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "submit_report should not emit events (none defined in events.rs)"
    );

    // verify_report
    let before = env.events().all();
    client.verify_report(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "verify_report should not emit events (none defined in events.rs)"
    );

    // dispute_report
    let reason = dispute_reason(&env);
    let before = env.events().all();
    client.dispute_report(&admin, &report_id, &reason);
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "dispute_report should not emit events (none defined in events.rs)"
    );

    // resolve_dispute
    let before = env.events().all();
    client.resolve_dispute(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "resolve_dispute should not emit events (none defined in events.rs)"
    );

    // update_report_status
    let before = env.events().all();
    client.update_report_status(&admin, &report_id, &common::status(&env));
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "update_report_status should not emit events (none defined in events.rs)"
    );

    // delete_report
    let before = env.events().all();
    client.delete_report(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        after.len(),
        before.len(),
        "delete_report should not emit events (none defined in events.rs)"
    );
}

#[test]
fn test_paused_entrypoints_fail_and_emit_no_events() {
    let (env, client, admin) = register_and_init();
    let reporter = common::reporter(&env);
    let data = report_data(&env);
    let hash = report_hash(&env);
    let reason = dispute_reason(&env);

    // Create a report before pausing so we have a valid report id for the
    // entrypoints that require one.
    let report_id = client.submit_report(&reporter, &data, &hash);

    client.pause_reporting(&admin);

    // submit_report
    let before = env.events().all();
    let result = client.try_submit_report(&reporter, &data, &hash);
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "submit_report should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed submit_report should not emit events"
    );

    // verify_report
    let before = env.events().all();
    let result = client.try_verify_report(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "verify_report should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed verify_report should not emit events"
    );

    // dispute_report
    let before = env.events().all();
    let result = client.try_dispute_report(&admin, &report_id, &reason);
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "dispute_report should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed dispute_report should not emit events"
    );

    // resolve_dispute
    let before = env.events().all();
    let result = client.try_resolve_dispute(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "resolve_dispute should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed resolve_dispute should not emit events"
    );

    // update_report_status
    let before = env.events().all();
    let result = client.try_update_report_status(&admin, &report_id, &common::status(&env));
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "update_report_status should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed update_report_status should not emit events"
    );

    // delete_report
    let before = env.events().all();
    let result = client.try_delete_report(&admin, &report_id);
    let after = env.events().all();
    assert_eq!(
        result,
        Err(Ok(ReportingError::ReportingPaused)),
        "delete_report should fail when paused"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "failed delete_report should not emit events"
    );
}