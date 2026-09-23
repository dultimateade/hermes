#![cfg(test)]

//! Ensures that [`ReportingError`] variant discriminants remain stable across
//! contract versions.
//!
//! Each discriminant is part of the client-facing contract API. Clients may
//! persist these numbers or use them to decode failed invocations, so existing
//! values must not be renumbered or reused. Add new variants with an explicit,
//! previously unused value.

use reporting::ReportingError;

#[test]
fn test_error_variant_stability() {
    assert_eq!(ReportingError::Unauthorized as u32, 1);
    assert_eq!(ReportingError::NotInitialized as u32, 2);
    assert_eq!(ReportingError::AlreadyInitialized as u32, 3);
    assert_eq!(ReportingError::ReportingPaused as u32, 4);
    assert_eq!(ReportingError::InvalidAdmin as u32, 5);
    assert_eq!(ReportingError::InvalidReporter as u32, 6);
    assert_eq!(ReportingError::ReportNotFound as u32, 7);
    assert_eq!(ReportingError::DisputeNotFound as u32, 8);
    assert_eq!(ReportingError::InvalidNewOwner as u32, 9);
}

#[test]
fn test_debug_format_does_not_panic() {
    // Just ensuring the derived Debug impl works for all variants.
    let _ = format!("{:?}", ReportingError::Unauthorized);
    let _ = format!("{:?}", ReportingError::NotInitialized);
    let _ = format!("{:?}", ReportingError::AlreadyInitialized);
    let _ = format!("{:?}", ReportingError::ReportingPaused);
    let _ = format!("{:?}", ReportingError::InvalidAdmin);
    let _ = format!("{:?}", ReportingError::InvalidReporter);
    let _ = format!("{:?}", ReportingError::ReportNotFound);
    let _ = format!("{:?}", ReportingError::DisputeNotFound);
    let _ = format!("{:?}", ReportingError::InvalidNewOwner);
}
