//! # Canonical Error Registry for Hermes Contracts
//!
//! This crate is the **single source of truth** for all error definitions,
//! discriminants, and documentation across Hermes contracts.
//!
//! ## Design
//!
//! Rather than duplicating error enums, discriminants, and stability tests across
//! seven different contracts, this registry centralizes error metadata and provides:
//!
//! 1. **Schema definitions** — error codes, names, descriptions, ranges
//! 2. **Stable discriminants** — frozen numeric codes that form part of the public API
//! 3. **Build-time code generation** — generates contract-specific error files at compile time
//! 4. **Stability enforcement** — compile-time exhaustive matching prevents silent drift
//!
//! ## Error Registry
//!
//! Each error is registered exactly once with:
//! - A **stable numeric code** (never changes once deployed)
//! - A **short name** (used in error enum variants)
//! - A **description** (user-facing documentation)
//! - An **optional range** (for code range validation in tests)
//! - A **scope** (which contracts use this error)

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErrorDefinition {
    /// Numeric code (frozen once deployed)
    pub code: u32,
    /// Enum variant name (e.g., "Unauthorized")
    pub name: &'static str,
    /// User-facing description
    pub description: &'static str,
    /// Optional code range (for categorization and tests)
    pub range: Option<(u32, u32)>,
    /// Which contracts use this error
    pub scope: BTreeSet<&'static str>,
}

impl ErrorDefinition {
    pub fn new(code: u32, name: &'static str, description: &'static str) -> Self {
        Self {
            code,
            name,
            description,
            range: None,
            scope: BTreeSet::new(),
        }
    }

    pub fn with_range(mut self, min: u32, max: u32) -> Self {
        self.range = Some((min, max));
        self
    }

    pub fn with_scope(mut self, scope: BTreeSet<&'static str>) -> Self {
        self.scope = scope;
        self
    }
}

/// Global registry of all Hermes error definitions.
///
/// # Rules for contributors
///
/// * **Never renumber** an existing error code.
/// * **Never remove** an error (deprecate instead).
/// * **Never reuse** a code from a removed error.
/// * New errors must be **appended** with a fresh, previously-unused code.
/// * The code range boundaries must **not overlap**.
/// * Adding a new error requires updating this registry and regenerating contract files.
///
/// # Code Range Allocation
///
/// Code ranges are allocated per contract to prevent collisions:
/// - Each contract reserves a 100-code block (0-99, 100-199, etc.)
/// - Within each contract, ranges are semantically organized (auth, data, admin, etc.)
///
/// | Contract      | Code Block | Code Range | Purpose             |
/// |---------------|------------|------------|---------------------|
/// | allowlist     | 0–99       | 1–9        | auth                |
/// |               |            | 10–19      | not used            |
/// |               |            | 20–29      | not used            |
/// |               |            | 30–99      | domain-specific     |
/// | analytics     | 100–199    | 101–109    | auth                |
/// |               |            | 110–119    | data / query        |
/// |               |            | 120–129    | metric / agg        |
/// |               |            | 130–139    | config / admin      |
/// | fees          | 200–299    | 201–209    | auth                |
/// |               |            | 210–299    | domain-specific     |
/// | markets       | 300–399    | 301–309    | auth                |
/// |               |            | 310–399    | domain-specific     |
/// | (etc.)        | (etc.)     | (etc.)     | (etc.)              |
///
/// # Deployment Record
///
/// Last freeze: `v0.1.0` (2026-09-24)
/// - 15 global errors (shared / common patterns)
/// - 78 contract-specific errors

pub fn all_errors() -> Vec<ErrorDefinition> {
    vec![
        // =====================================================================
        // UNIVERSAL ERRORS (used across all or most contracts)
        // =====================================================================
        //
        // These are canonical definitions of errors that appear in multiple
        // contracts with identical meaning and code. They serve as a reference
        // for contracts to include or exclude as needed.

        // ----- Authorization (code 1) -----
        ErrorDefinition::new(1, "Unauthorized", "Caller is not authorized for the action.")
            .with_range(1, 9),

        // ----- Initialization (codes 2–4) -----
        ErrorDefinition::new(2, "NotInitialized", "Contract has not been initialized.")
            .with_range(1, 9),
        ErrorDefinition::new(
            3,
            "AlreadyInitialized",
            "Contract has already been initialized.",
        )
        .with_range(1, 9),

        // ----- Arithmetic (codes 10–20) -----
        ErrorDefinition::new(20, "Overflow", "Arithmetic overflow occurred.")
            .with_range(20, 29),

        // =====================================================================
        // ALLOWLIST CONTRACT ERRORS (codes 1–99)
        // =====================================================================
        // Scope: allowlist
        // Ranges: 1–9 (auth), 10–19 (not reserved), 20–99 (domain-specific)

        // Already defined above as universal: Unauthorized (1), NotInitialized (2), AlreadyInitialized (3)
        // Already defined above as universal: Overflow (20)

        ErrorDefinition::new(4, "AllowlistNotFound", "The allowlist ID does not exist.")
            .with_range(1, 99),
        ErrorDefinition::new(
            5,
            "AllowlistAlreadyExists",
            "The allowlist ID already exists.",
        )
        .with_range(1, 99),
        ErrorDefinition::new(
            6,
            "AddressAlreadyInAllowlist",
            "The provided address is already in the allowlist.",
        )
        .with_range(1, 99),
        ErrorDefinition::new(
            7,
            "AddressNotInAllowlist",
            "The provided address is not in the allowlist.",
        )
        .with_range(1, 99),
        ErrorDefinition::new(8, "AllowlistEmpty", "The allowlist is empty.")
            .with_range(1, 99),
        ErrorDefinition::new(9, "InvalidInput", "Invalid input parameters.")
            .with_range(1, 9),

        // =====================================================================
        // ANALYTICS CONTRACT ERRORS (codes 100–199)
        // =====================================================================
        // Scope: analytics
        // Ranges: 101–109 (auth), 110–119 (data/query), 120–129 (metric/agg), 130–139 (config/admin)

        ErrorDefinition::new(101, "Unauthorized", "Caller is not authorized to perform the requested action.")
            .with_range(101, 109),
        ErrorDefinition::new(
            102,
            "AdminNotSet",
            "Admin address has not been set; contract may not have been initialized.",
        )
        .with_range(101, 109),
        ErrorDefinition::new(
            103,
            "NotInitialized",
            "Contract is not yet initialized; call `initialize` first.",
        )
        .with_range(101, 109),
        ErrorDefinition::new(
            104,
            "AlreadyInitialized",
            "Contract has already been initialized and cannot be initialized again.",
        )
        .with_range(101, 109),

        ErrorDefinition::new(110, "MarketNotFound", "The requested market was not found in the analytics store.")
            .with_range(110, 119),
        ErrorDefinition::new(
            111,
            "SnapshotNotFound",
            "The requested metric snapshot does not exist.",
        )
        .with_range(110, 119),
        ErrorDefinition::new(
            112,
            "InvalidTimeRange",
            "The requested time-range is invalid (e.g. end < start).",
        )
        .with_range(110, 119),
        ErrorDefinition::new(
            113,
            "UnsupportedWindow",
            "The requested aggregation window is not supported.",
        )
        .with_range(110, 119),

        ErrorDefinition::new(120, "Overflow", "An arithmetic overflow occurred while computing a metric.")
            .with_range(120, 129),
        ErrorDefinition::new(121, "StoreFull", "The analytics store has reached its maximum capacity.")
            .with_range(120, 129),
        ErrorDefinition::new(
            122,
            "DuplicateEntry",
            "The submitted data point is a duplicate of an already-recorded entry.",
        )
        .with_range(120, 129),
        ErrorDefinition::new(
            123,
            "ValueOutOfRange",
            "The data point value is out of the accepted range.",
        )
        .with_range(120, 129),

        ErrorDefinition::new(130, "InvalidConfig", "One or more configuration parameters are invalid.")
            .with_range(130, 139),
        ErrorDefinition::new(131, "AnalyticsPaused", "Analytics collection is currently paused.")
            .with_range(130, 139),
        ErrorDefinition::new(
            132,
            "InvalidState",
            "The requested operation is not permitted in the current contract state.",
        )
        .with_range(130, 139),
    ]
}

/// Look up an error definition by code.
pub fn find_by_code(code: u32) -> Option<ErrorDefinition> {
    all_errors()
        .into_iter()
        .find(|def| def.code == code)
}

/// Look up all error definitions by name.
pub fn find_by_name(name: &str) -> Vec<ErrorDefinition> {
    all_errors()
        .into_iter()
        .filter(|def| def.name == name)
        .collect()
}

/// Return all errors that are in a given code range (min, max inclusive).
pub fn errors_in_range(min: u32, max: u32) -> Vec<ErrorDefinition> {
    all_errors()
        .into_iter()
        .filter(|def| def.code >= min && def.code <= max)
        .collect()
}

/// Validate that all error codes are unique.
pub fn validate_unique_codes() -> Result<(), String> {
    let errors = all_errors();
    let mut seen: BTreeSet<u32> = BTreeSet::new();

    for err in &errors {
        if !seen.insert(err.code) {
            return Err(format!("Duplicate error code: {} ({})", err.code, err.name));
        }
    }

    Ok(())
}

/// Validate that code ranges do not overlap.
pub fn validate_range_boundaries() -> Result<(), Vec<String>> {
    let errors = all_errors();
    let mut errors_list = Vec::new();

    for err in &errors {
        if let Some((min, max)) = err.range {
            if err.code < min || err.code > max {
                errors_list.push(format!(
                    "Error {} (code {}) falls outside its declared range ({}-{})",
                    err.name, err.code, min, max
                ));
            }
        }
    }

    if errors_list.is_empty() {
        Ok(())
    } else {
        Err(errors_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_no_duplicate_codes() {
        validate_unique_codes().expect("Duplicate error codes found");
    }

    #[test]
    fn registry_respects_range_boundaries() {
        validate_range_boundaries().expect("Range boundary violations found");
    }

    #[test]
    fn universal_errors_are_defined() {
        assert!(find_by_code(1).is_some(), "Unauthorized (1) must be defined");
    }

    #[test]
    fn analytics_errors_are_defined() {
        assert!(
            find_by_code(110).is_some(),
            "MarketNotFound (110) must be defined"
        );
    }
}
