#![cfg(test)]

use admin::ContractError;

#[test]
fn admin_error_discriminants_are_stable() {
    let errors = [
        (ContractError::AlreadyInitialized, 1u32),
        (ContractError::AdminNotSet, 2u32),
        (ContractError::Unauthorized, 3u32),
        (ContractError::AdminActionTimelocked, 4u32),
        (ContractError::InvalidCooldown, 5u32),
        (ContractError::InvalidNewOwner, 6u32),
        (ContractError::OwnershipTransferNotProposed, 7u32),
        (ContractError::OwnershipTransferTimelocked, 8u32),
    ];

    for (error, expected) in errors {
        assert_eq!(
            error as u32,
            expected,
            "admin error discriminant changed for {error:?}"
        );
    }
}

#[test]
fn admin_error_discriminants_are_unique() {
    let codes = [
        ContractError::AlreadyInitialized as u32,
        ContractError::AdminNotSet as u32,
        ContractError::Unauthorized as u32,
        ContractError::AdminActionTimelocked as u32,
        ContractError::InvalidCooldown as u32,
        ContractError::InvalidNewOwner as u32,
        ContractError::OwnershipTransferNotProposed as u32,
        ContractError::OwnershipTransferTimelocked as u32,
    ];

    for (index, code) in codes.iter().enumerate() {
        assert_eq!(*code, (index + 1) as u32);
        assert!(!codes[..index].contains(code));
    }
}
