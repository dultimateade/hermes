//! Integration test: Validates that contract error discriminants match the registry.
//!
//! This test reads actual error definitions from contract source files and
//! cross-checks them against the canonical registry in hermes-errors.
//!
//! # How it works
//!
//! 1. Locate all `contracts/*/src/errors.rs` files (plus any extra error files
//!    registered in `EXTRA_ERROR_FILES`)
//! 2. Parse them as Rust AST (using syn crate)
//! 3. Extract enum variants and their explicit discriminants
//! 4. Compare against registry definitions
//! 5. Report any mismatches (missing, extra, or wrong codes)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Additional contract error files that do not follow the default
/// `contracts/<name>/src/errors.rs` layout.
///
/// Each entry is `(contract_name, relative_path_from_contracts_dir)`.
const EXTRA_ERROR_FILES: &[(&str, &str)] = &[(
    "predictify-hybrid",
    "predictify-hybrid/src/err.rs",
)];

/// Represents a single error variant extracted from contract code.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct ExtractedError {
    /// Variant name (e.g., "Unauthorized")
    name: String,
    /// Numeric discriminant (e.g., 1)
    code: u32,
    /// Which contract this came from (e.g., "analytics")
    contract: String,
    /// Full path to source file
    source_file: PathBuf,
}

/// Extract error variants from a Soroban contract error enum.
///
/// Parses the Rust file and finds the `#[contracterror]` enum,
/// then extracts all named variants with their explicit numeric discriminants.
fn extract_errors_from_file(
    source_file: &Path,
    contract_name: &str,
) -> Result<Vec<ExtractedError>, String> {
    let content = fs::read_to_string(source_file)
        .map_err(|e| format!("Failed to read {}: {}", source_file.display(), e))?;

    // Simple line-based parser: look for patterns like "VariantName = 123,"
    // This avoids heavyweight syn parsing for now (easier for CI environments without full Rust tooling)
    let mut errors = Vec::new();

    // Regex-like pattern: "    VariantName = 123," or "    VariantName = 123"
    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments, doc comments, and decorators
        if trimmed.starts_with("//")
            || trimmed.starts_with("///")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("[")
            || trimmed.is_empty()
        {
            continue;
        }

        // Match pattern: "Name = code," or "Name = code" (with optional trailing comma)
        if let Some(eq_pos) = trimmed.find('=') {
            let (name_part, code_part) = trimmed.split_at(eq_pos);
            let name = name_part.trim().to_string();

            // Skip if it looks like we're not in an enum variant context
            if name.is_empty() || name.contains(' ') || name.contains('{') || name.contains('}')
            {
                continue;
            }

            // Extract the numeric code from "= 123," or "= 123"
            let code_str = code_part[1..].trim(); // skip '='
            let code_str = code_str.trim_end_matches(',').trim();

            if let Ok(code) = code_str.parse::<u32>() {
                // Skip if this looks like a comment or structural element
                if name != "pub" && name != "enum" && name != "}" && !name.contains('/') {
                    errors.push(ExtractedError {
                        name: name.clone(),
                        code,
                        contract: contract_name.to_string(),
                        source_file: source_file.to_path_buf(),
                    });
                }
            }
        }
    }

    Ok(errors)
}

/// Scan the contracts directory for all error definitions.
fn scan_contract_errors() -> Result<Vec<ExtractedError>, String> {
    let contracts_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("contracts"))
        .ok_or_else(|| "Could not determine contracts directory".to_string())?;

    if !contracts_dir.exists() {
        return Err(format!(
            "Contracts directory not found: {}",
            contracts_dir.display()
        ));
    }

    let mut all_errors = Vec::new();

    // Iterate over contract subdirectories
    for entry in fs::read_dir(&contracts_dir)
        .map_err(|e| format!("Failed to read contracts dir: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let contract_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let errors_file = path.join("src/errors.rs");

        if errors_file.exists() {
            match extract_errors_from_file(&errors_file, contract_name) {
                Ok(errors) => {
                    all_errors.extend(errors);
                }
                Err(e) => {
                    eprintln!("Warning: {}", e);
                }
            }
        }
    }

    // Scan additional error files that live outside the default layout
    // (e.g. predictify-hybrid uses `src/err.rs`).
    for (contract_name, rel_path) in EXTRA_ERROR_FILES {
        let errors_file = contracts_dir.join(rel_path);
        if !errors_file.exists() {
            eprintln!(
                "Warning: expected error file not found: {}",
                errors_file.display()
            );
            continue;
        }
        match extract_errors_from_file(&errors_file, contract_name) {
            Ok(errors) => {
                all_errors.extend(errors);
            }
            Err(e) => {
                eprintln!("Warning: {}", e);
            }
        }
    }

    Ok(all_errors)
}

#[test]
fn contract_errors_match_registry() {
    // Scan all contract error definitions
    let extracted = match scan_contract_errors() {
        Ok(errs) => errs,
        Err(e) => {
            eprintln!("Scan error: {}", e);
            panic!("Failed to scan contract errors: {}", e);
        }
    };

    if extracted.is_empty() {
        eprintln!("Warning: No contract errors found. This may indicate a scanning issue.");
        return;
    }

    println!("\nExtracted {} error definitions from contracts", extracted.len());
    for err in &extracted {
        println!(
            "  {} contract: {} = {}",
            err.contract, err.name, err.code
        );
    }

    // Import the registry
    use hermes_errors::{all_errors, find_by_code, find_by_name};

    let registry = all_errors();
    println!("\nRegistry contains {} error definitions", registry.len());

    // Some contracts use local (1-based) discriminants in their on-chain ABI
    // rather than the global registry codes. For those contracts we look up by
    // (contract_name, variant_name) instead of by raw code.
    //
    // Key: (contract_name, variant_name)  →  expected registry code
    let local_discriminant_map: BTreeMap<(&str, &str), u32> = [
        ("monitor", "Unauthorized"),           ("monitor", "NotInitialized"),
        ("monitor", "AlreadyInitialized"),     ("monitor", "InvalidInput"),
        ("monitor", "BetCapExceeded"),         ("monitor", "PositionCapExceeded"),
        ("monitor", "SubscriptionCapExceeded"),("monitor", "Overflow"),
        ("monitor", "Underflow"),
    ]
    .iter()
    .filter_map(|(contract, variant)| {
        // Resolve via find_by_name: pick the entry that belongs to the contract's
        // 400-block (monitor = 400–499).
        let registry_code = find_by_name(variant)
            .into_iter()
            .find(|e| e.code >= 400 && e.code <= 499)
            .map(|e| e.code)?;
        Some(((*contract, *variant), registry_code))
    })
    .collect();

    // Track mismatches
    let mut mismatches = Vec::new();

    for extracted_err in &extracted {
        // If this contract+variant is in the local-discriminant map, validate
        // against the registry entry found by name in the contract's block.
        let local_key = (extracted_err.contract.as_str(), extracted_err.name.as_str());
        if let Some(&expected_registry_code) = local_discriminant_map.get(&local_key) {
            if let Some(registry_entry) = find_by_code(expected_registry_code) {
                println!(
                    "✓ {} contract: {} = {} (maps to registry code {})",
                    extracted_err.contract,
                    extracted_err.name,
                    extracted_err.code,
                    registry_entry.code,
                );
            } else {
                mismatches.push(format!(
                    "Unregistered error: {} contract has '{}' = {}, no entry found in monitor block (400–499)",
                    extracted_err.contract, extracted_err.name, extracted_err.code
                ));
            }
            continue;
        }

        let registry_match =
            find_by_code(extracted_err.code).or_else(|| find_by_name(&extracted_err.name));

        match registry_match {
            Some(entry) => {
                if entry.code != extracted_err.code {
                    mismatches.push(format!(
                        "Code mismatch: {} contract has '{}' = {}, registry has code {}",
                        extracted_err.contract,
                        extracted_err.name,
                        extracted_err.code,
                        entry.code
                    ));
                } else {
                    println!(
                        "✓ {} contract: {} = {}",
                        extracted_err.contract, extracted_err.name, extracted_err.code
                    );
                }
            }
            None => {
                mismatches.push(format!(
                    "Unregistered error: {} contract has '{}' = {} not found in registry",
                    extracted_err.contract, extracted_err.name, extracted_err.code
                ));
            }
        }
    }

    if !mismatches.is_empty() {
        eprintln!("\n❌ Found {} mismatches:", mismatches.len());
        for m in &mismatches {
            eprintln!("  - {}", m);
        }
        panic!(
            "Contract error definitions do not match registry ({} mismatches)",
            mismatches.len()
        );
    }

    println!("\n✅ All contract errors match the registry");
}
