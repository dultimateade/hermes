# Error Documentation Validation Test

## Overview

The `validate_contract_errors` integration test ensures that all error discriminants defined in contract source files (`contracts/*/src/errors.rs`) are registered in the canonical error registry (`crates/hermes-errors/src/lib.rs`).

This prevents silent drift between:
- **Source of truth**: Registry definitions in `hermes-errors/src/lib.rs`
- **Implementation**: Actual error enums in contract code
- **Documentation**: README and user-facing error catalogs

## Running the Test

### From the project root:

```bash
cargo test -p hermes-errors --test validate_contract_errors
```

### From the hermes-errors crate directory:

```bash
cd crates/hermes-errors
cargo test --test validate_contract_errors
```

### With verbose output:

```bash
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

## What the Test Validates

The test scans all contract error files and validates:

1. **All error codes in contracts are registered**: Every discriminant found in `contracts/*/src/errors.rs` must have a matching entry in the registry
2. **Names match**: If a contract defines `Unauthorized = 1`, the registry must also define `Unauthorized` at code `1`
3. **No ghost errors**: The registry may define errors not yet deployed to contracts (marked as undeployed warnings)
4. **Exhaustive coverage**: All six contracts' error files are scanned: `analytics`, `fees`, `markets`, `monitor`, `resolution`, `validators`

## How It Works

### Parsing Strategy

The test uses a **line-based parser** (not full AST parsing) to extract errors:

- Scans each contract's `src/errors.rs` file line-by-line
- Identifies lines matching pattern: `VariantName = code,` (with optional trailing comma)
- Skips comments (`//`, `///`), decorators (`#[`), and structural elements (`enum`, `}`}
- Extracts variant name and numeric discriminant

**Why line-based parsing?** It avoids dependency on heavy parsing libraries and works in any Rust environment without requiring syn/quote to be imported.

### Comparison Logic

1. For each extracted error: lookup by code in registry
2. Verify the name matches exactly
3. Report mismatches:
   - **Unregistered error**: Contract has code but registry doesn't
   - **Name conflict**: Same code but different variant name
   - **Undeployed**: Registry has code but no contract uses it yet (info, not error)

## Integration with CI

### Add to CI Pipeline

Create a step in `.github/workflows/test.yml`:

```yaml
- name: Validate error documentation
  run: cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

### Pre-commit Hook

Add to `.git/hooks/pre-commit`:

```bash
#!/bin/bash
cargo test -p hermes-errors --test validate_contract_errors --quiet
if [ $? -ne 0 ]; then
  echo "Error documentation out of sync with registry. Run:"
  echo "  cargo test -p hermes-errors --test validate_contract_errors -- --nocapture"
  exit 1
fi
```

## Output Examples

### Success ✓

```
Extracted 32 error definitions from contracts
  analytics contract: Unauthorized = 101
  analytics contract: MarketNotFound = 110
  ... (more lines)

Registry contains 32 error definitions

✓ analytics contract: Unauthorized = 101 (matches registry)
✓ analytics contract: MarketNotFound = 110 (matches registry)
... (more lines)

✓ All contract errors match the registry!
```

### Failure ❌

```
Extracted 34 error definitions from contracts
  analytics contract: Unauthorized = 101
  analytics contract: NewUnregisteredError = 150
  ... (more lines)

Registry contains 32 error definitions

✓ analytics contract: Unauthorized = 101 (matches registry)
❌ Unregistered error: analytics contract has 'NewUnregisteredError' = 150, 
   but this code is not in the registry

Error definitions do not match registry!

thread 'contract_errors_match_registry' panicked at 'Error definitions do not match registry!'
```

## Fixing Mismatches

### If a contract defines a new error not in the registry:

1. Add the error to `crates/hermes-errors/src/lib.rs`:

```rust
ErrorDefinition::new(150, "NewUnregisteredError", "Description of the error.")
    .with_range(150, 159),
```

2. Run the registry validation:

```bash
cargo test -p hermes-errors
```

3. Re-run the contract error test:

```bash
cargo test -p hermes-errors --test validate_contract_errors
```

### If a contract's discriminant doesn't match the registry:

This indicates either:
- A contract was deployed with a different discriminant than the registry expects, or
- The registry was updated but the contract wasn't regenerated

Check the contract's `build.rs` or source generation process to ensure it's using the latest registry.

## Design Rationale

### Why a separate test instead of a build script?

1. **Build scripts cannot fail** — they can only warn. A hard test failure ensures the issue is caught in CI.
2. **Decoupling** — separates registry validation from code generation. The registry can be validated independently.
3. **Clarity** — test output is explicit and designed for developers; build script output is often buried.

### Why line-based parsing instead of syn?

1. **Zero dependencies** — avoids adding `syn` and `quote` to test dependencies.
2. **Speed** — line-based parsing is faster than full AST parsing.
3. **Robustness** — works even if Soroban's macro syntax evolves.
4. **Simplicity** — easier to understand and maintain than AST traversal.

### Why compare against the registry instead of the README?

1. **Single source of truth** — the registry in code is authoritative.
2. **Programmatic** — easier to validate code than to parse markdown tables.
3. **Extensibility** — the registry can drive code generation, export formats (JSON, GraphQL), and other tooling.

## Future Enhancements

- [ ] **Automatic registry updates** — detect unregistered errors and prompt for registry update
- [ ] **Registry generation from contracts** — reverse-engineer registry from deployed contract hashes (for migrations)
- [ ] **Error stability snapshots** — store per-release error codes for audit trails
- [ ] **Cross-contract error linking** — document when one contract's error is caused by another's validation failure

## Related Files

- `crates/hermes-errors/src/lib.rs` — Canonical registry and validation functions
- `crates/hermes-errors/tests/validate_contract_errors.rs` — This test
- `contracts/*/src/errors.rs` — Contract error definitions (what this test validates)
- `docs/impl/CONTRIBUTING_ERRORS.md` — Guide for adding/modifying errors
- `docs/impl/MIGRATION_ERROR_SYNC.md` — Migrating contracts to use the registry

## Troubleshooting

### Test can't find contracts directory

The test uses `CARGO_MANIFEST_DIR` to locate the hermes-errors crate, then navigates up two levels to find the `contracts/` directory. If the directory structure changes, update the path calculation in `scan_contract_errors()`.

### Test finds no errors

If the parser finds zero errors, check:

1. Contracts directory exists at expected location
2. `src/errors.rs` files have the expected structure:
   ```rust
   #[contracterror]
   pub enum ContractError {
       VariantName = 1,
       // ...
   }
   ```
3. Variant names and discriminants are on the same line (or at least, `= code` is on the same line as the variant name)

### False positives: "Unregistered error" warnings

Some contracts may define errors that aren't in the registry yet—that's expected during development. The test outputs these as warnings (⚠) rather than failures. Only actual code/name mismatches fail the test.

---

## Questions?

See:
- `docs/impl/CONTRIBUTING_ERRORS.md` — Workflow for adding/modifying errors
- `docs/impl/ERROR_DOCS_SYNC.md` — Overall error architecture
