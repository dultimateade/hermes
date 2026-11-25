//! # Error discriminant stability tests
//!
//! These tests pin the numeric value of every [`MonitorError`] variant.
//!
//! They are intentionally brittle: if a discriminant changes, these tests
//! fail immediately, alerting the author that they have introduced a breaking
//! change to the on-chain ABI.
//!
//! `test_all_variants_are_covered` uses an exhaustive `match` so that adding
//! a new variant to `MonitorError` without updating this file causes a
//! **compile error**, not a silent gap in coverage.

#![cfg(test)]

use monitor::MonitorError;

#[test]
fn test_error_discriminants_are_stable() {
    assert_eq!(MonitorError::Unauthorized as u32, 1);
    assert_eq!(MonitorError::NotInitialized as u32, 2);
    assert_eq!(MonitorError::AlreadyInitialized as u32, 3);
    assert_eq!(MonitorError::BetCapExceeded as u32, 4);
    assert_eq!(MonitorError::PositionCapExceeded as u32, 5);
    assert_eq!(MonitorError::SubscriptionCapExceeded as u32, 6);
    assert_eq!(MonitorError::InvalidInput as u32, 7);
    assert_eq!(MonitorError::Overflow as u32, 8);
    assert_eq!(MonitorError::Underflow as u32, 9);
}

#[test]
fn test_debug_format_does_not_panic() {
    let _ = format!("{:?}", MonitorError::Unauthorized);
    let _ = format!("{:?}", MonitorError::NotInitialized);
    let _ = format!("{:?}", MonitorError::AlreadyInitialized);
    let _ = format!("{:?}", MonitorError::BetCapExceeded);
    let _ = format!("{:?}", MonitorError::PositionCapExceeded);
    let _ = format!("{:?}", MonitorError::SubscriptionCapExceeded);
    let _ = format!("{:?}", MonitorError::InvalidInput);
    let _ = format!("{:?}", MonitorError::Overflow);
    let _ = format!("{:?}", MonitorError::Underflow);
}

/// Exhaustive match over every `MonitorError` variant.
///
/// This function is **not a test** — it never runs.  Its sole purpose is to
/// produce a compile error if a new variant is added to `MonitorError` without
/// a corresponding arm here.  Add the new arm, pin its discriminant in
/// `test_error_discriminants_are_stable`, and add its `Debug` call in
/// `test_debug_format_does_not_panic`.
#[allow(dead_code)]
fn assert_exhaustive_match(e: MonitorError) {
    match e {
        MonitorError::Unauthorized => {}
        MonitorError::NotInitialized => {}
        MonitorError::AlreadyInitialized => {}
        MonitorError::BetCapExceeded => {}
        MonitorError::PositionCapExceeded => {}
        MonitorError::SubscriptionCapExceeded => {}
        MonitorError::InvalidInput => {}
        MonitorError::Overflow => {}
        MonitorError::Underflow => {}
    }
}
