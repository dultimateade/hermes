//! Build script for Analytics contract.
//!
//! Generates contract-specific error files from the canonical error registry
//! in `crates/hermes-errors/`. This ensures the error enum, stability tests,
//! and documentation are always in sync.
//!
//! To regenerate error files:
//!
//! ```bash
//! cd contracts/analytics
//! cargo build
//! ```

use hermes_codegen::GenerateErrors;
use hermes_errors::all_errors;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Rerun if the error registry changes
    println!("cargo:rerun-if-changed=../../crates/hermes-errors/src/lib.rs");

    // Filter errors for this contract
    let analytics_errors: Vec<_> = all_errors()
        .into_iter()
        .filter(|e| {
            // Include universal errors and analytics-specific errors
            e.code == 1       // Unauthorized (universal)
                || (e.code >= 100 && e.code < 200) // Analytics block
        })
        .collect();

    // Generate error files
    GenerateErrors::new("analytics", &analytics_errors)
        .generate_errors_rs(&out_dir);
    // Uncomment to also generate stability tests:
    // .generate_err_stab_rs(&out_dir);
}
