#![cfg(test)]

//! Ensures that [`AllowlistError`] variants remain stable across contract
//! versions.
//!
//! Each discriminant is part of the client-facing contract API. Clients may
//! persist these numbers or use them to decode failed invocations, so existing
//! values must not be renumbered or reused. Add new variants with an explicit,
//! previously unused value.

use allowlist::AllowlistError;

const ALL_ERRORS: [AllowlistError; 11] = [
    AllowlistError::Unauthorized,
    AllowlistError::NotInitialized,
    AllowlistError::AlreadyInitialized,
    AllowlistError::AllowlistNotFound,
    AllowlistError::AllowlistAlreadyExists,
    AllowlistError::AddressAlreadyInAllowlist,
    AllowlistError::AddressNotInAllowlist,
    AllowlistError::AllowlistEmpty,
    AllowlistError::InvalidInput,
    AllowlistError::Overflow,
    AllowlistError::AllowlistExpired,
];

#[test]
fn test_error_variant_stability() {
    assert_eq!(AllowlistError::Unauthorized as u32, 1);
    assert_eq!(AllowlistError::NotInitialized as u32, 2);
    assert_eq!(AllowlistError::AlreadyInitialized as u32, 3);
    assert_eq!(AllowlistError::AllowlistNotFound as u32, 4);
    assert_eq!(AllowlistError::AllowlistAlreadyExists as u32, 5);
    assert_eq!(AllowlistError::AddressAlreadyInAllowlist as u32, 6);
    assert_eq!(AllowlistError::AddressNotInAllowlist as u32, 7);
    assert_eq!(AllowlistError::AllowlistEmpty as u32, 8);
    assert_eq!(AllowlistError::InvalidInput as u32, 9);
    assert_eq!(AllowlistError::Overflow as u32, 10);
    assert_eq!(AllowlistError::AllowlistExpired as u32, 11);
}

#[test]
fn test_debug_format_does_not_panic() {
    for error in ALL_ERRORS {
        let expected = match error {
            AllowlistError::Unauthorized => "Unauthorized",
            AllowlistError::NotInitialized => "NotInitialized",
            AllowlistError::AlreadyInitialized => "AlreadyInitialized",
            AllowlistError::AllowlistNotFound => "AllowlistNotFound",
            AllowlistError::AllowlistAlreadyExists => "AllowlistAlreadyExists",
            AllowlistError::AddressAlreadyInAllowlist => "AddressAlreadyInAllowlist",
            AllowlistError::AddressNotInAllowlist => "AddressNotInAllowlist",
            AllowlistError::AllowlistEmpty => "AllowlistEmpty",
            AllowlistError::InvalidInput => "InvalidInput",
            AllowlistError::Overflow => "Overflow",
            AllowlistError::AllowlistExpired => "AllowlistExpired",
        };
        assert_eq!(format!("{error:?}"), expected);
    }
}

#[test]
fn test_error_count_and_unique_discriminants() {
    assert_eq!(ALL_ERRORS.len(), 11);

    let mut discriminants: Vec<u32> = ALL_ERRORS.iter().map(|error| *error as u32).collect();
    discriminants.sort_unstable();
    discriminants.dedup();
    assert_eq!(
        discriminants.len(),
        ALL_ERRORS.len(),
        "Error variants should have unique discriminants"
    );
}
