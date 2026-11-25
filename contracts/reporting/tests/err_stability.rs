#![cfg(test)]

//! Ensures that [`ReportingError`] variant discriminants remain stable across
//! contract versions.
//!
//! Each discriminant is part of the client-facing contract API. Clients may
//! persist these numbers or use them to decode failed invocations, so existing
//! values must not be renumbered or reused. Add new variants with an explicit,
//! previously unused value.

use reporting::ReportingError;

fn expected_discriminant(error: ReportingError) -> u32 {
    match error {
        ReportingError::Unauthorized => 1,
        ReportingError::NotInitialized => 2,
        ReportingError::AlreadyInitialized => 3,
        ReportingError::ReportingPaused => 4,
        ReportingError::InvalidAdmin => 5,
        ReportingError::InvalidReporter => 6,
        ReportingError::ReportNotFound => 7,
        ReportingError::DisputeNotFound => 8,
        ReportingError::InvalidNewOwner => 9,
    }
}

#[test]
fn test_error_variant_stability() {
    for error in [
        ReportingError::Unauthorized,
        ReportingError::NotInitialized,
        ReportingError::AlreadyInitialized,
        ReportingError::ReportingPaused,
        ReportingError::InvalidAdmin,
        ReportingError::InvalidReporter,
        ReportingError::ReportNotFound,
        ReportingError::DisputeNotFound,
        ReportingError::InvalidNewOwner,
    ] {
        assert_eq!(error as u32, expected_discriminant(error));
    }
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
