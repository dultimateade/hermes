# Hermes Errors — Canonical Error Registry

This crate is the **single source of truth** for all error definitions, codes, and documentation across Hermes contracts.

## Problem Solved

Previously, error documentation existed in **three divergent copies**:

1. **Enum definitions** (`contracts/*/src/errors.rs`) — Rust code
2. **Stability tests** (`contracts/*/tests/err_stab.rs`) — Manual snapshots
3. **Comment headers** — Code range documentation

Without a sync mechanism, these inevitably drifted, causing:
- Silent discriminant mismatches
- Inconsistent error codes across contracts
- Failed deployments
- Outdated documentation

## Solution: Canonical Registry

This crate defines every error exactly once. Build-time code generation in each contract produces synchronized error files automatically.

**Guarantees:**
- ✅ Every error code defined once
- ✅ Numeric codes validated (unique, within ranges)
- ✅ No manual duplication
- ✅ Build-time enforcement
- ✅ Compile-time exhaustiveness checking

## Architecture

Three layers ensure single-source-of-truth:

```
1. Canonical Registry (this crate)
   └─ ErrorDefinition structs in src/lib.rs
      └─ cargo test validates uniqueness & ranges
   
2. Build-time Code Generation (hermes-codegen)
   └─ reads registry
   └─ filters errors per contract
   └─ generates src/errors.rs and tests/err_stab.rs
   
3. Contract Builds (contracts/*/build.rs)
   └─ runs code generation
   └─ includes generated files
   └─ validates at compile time
```

## Quick Start

### For End Users (Contract Developers)

See `CONTRIBUTING_ERRORS.md` for the workflow of adding/modifying errors.

TL;DR to add a new error:

```rust
// In crates/hermes-errors/src/lib.rs
ErrorDefinition::new(114, "NewError", "Description")
    .with_range(110, 119),
```

Then:

```bash
cargo test -p hermes-errors  # Validate
cargo build                   # Regenerate contract files
```

### For Maintainers (First-time Setup)

1. Registry is already defined in `src/lib.rs`
2. Code generation is in `crates/hermes-codegen/`
3. Migrate contracts one at a time using `MIGRATION_ERROR_SYNC.md`

## Code Ranges

Each contract reserves a 100-code block:

| Contract   | Block    | Ranges                        |
|------------|----------|-------------------------------|
| allowlist  | 0–99     | 1–9 (auth), 4–8 (domain)      |
| analytics  | 100–199  | 101–109 (auth), 110–119 (data), 120–129 (metrics), 130–139 (admin) |
| fees       | 200–299  | 201–209 (auth), rest (domain) |
| markets    | 300–399  | 301–309 (auth), rest (domain) |
| monitor    | 400–499  | 401–409 (auth), rest (domain) |
| resolution | 500–599  | 501–509 (auth), rest (domain) |
| validators | 600–699  | 601–609 (auth), rest (domain) |

Within each contract, ranges categorize errors semantically (auth, data, admin, etc.).

## Usage

### Reading the Registry

```rust
use hermes_errors::{all_errors, find_by_code, find_by_name};

// Get all errors
let errors = all_errors();

// Find by code
if let Some(err) = find_by_code(101) {
    println!("Error 101: {}", err.name);  // Output: Error 101: Unauthorized
}

// Find by name (multiple contracts may use same name)
let unauthorized_errors = find_by_name("Unauthorized");
for err in unauthorized_errors {
    println!("Code {}: {}", err.code, err.description);
}
```

### Validating the Registry

```rust
use hermes_errors::{validate_unique_codes, validate_range_boundaries};

// Ensure no duplicate codes
validate_unique_codes().expect("Duplicate codes found");

// Ensure codes respect their ranges
validate_range_boundaries().expect("Range violations found");
```

## Adding an Error

Edit `src/lib.rs` and add to `all_errors()`:

```rust
ErrorDefinition::new(
    222,  // Code
    "InsufficientBalance",  // Variant name
    "Account balance is below the required minimum."  // Description
)
.with_range(220, 229),  // Code range (optional)
```

Then validate:

```bash
cargo test
```

## File Structure

```
crates/hermes-errors/
├── Cargo.toml           # Package definition
├── build.rs             # Build validation (not code generation)
├── src/
│   └── lib.rs           # Canonical registry + validation functions
└── tests/
    └── integration_test.rs  # Registry validation tests (auto-generated)
```

## Testing

Run registry validation:

```bash
cargo test
```

Tests validate:

- **Uniqueness** — no duplicate codes
- **Range boundaries** — codes within declared ranges
- **Non-overlapping ranges** — ranges don't collide
- **Completion** — all errors accounted for

## Dependencies

- `std` — standard library only
- Optional: `serde` (for JSON export in future versions)

## Related Crates

- **hermes-codegen** — generates contract-specific error files from this registry
- **contracts/** — use generated files in their `build.rs`

## Design Principles

1. **Single Source of Truth** — Registry defines every error once
2. **Immutable Codes** — Once deployed, error codes never change
3. **Build-Time Enforcement** — Code generation happens at compile time
4. **Explicit Ranges** — Code ranges prevent collisions and organize errors
5. **Exhaustive Matching** — Macros ensure generated files are complete

## Stability Guarantee

Once an error code is deployed to production:

- ✅ **Never** renumber the code
- ✅ **Never** remove the error (deprecate instead)
- ✅ **Never** reuse the code for a different error

This is a **client-facing API contract** — clients depend on stable error codes for:
- Client SDKs and error handling
- Off-chain indexers and analytics dashboards
- Monitoring systems
- Smart contract bridges and integration points

Breaking this contract causes:
- Silent errors (clients misinterpret error codes)
- Data loss (off-chain systems lose sync)
- Failed integrations (bridges break)
- Production incidents

## Migration Path

For existing contracts not yet using the registry:

1. Inventory all current error codes
2. Add them to the registry in `src/lib.rs`
3. Validate with `cargo test`
4. Set up `build.rs` in each contract
5. Regenerate error files
6. Test and commit

See `MIGRATION_ERROR_SYNC.md` for detailed steps.

## Future Enhancements

- [ ] OpenAPI export (error definitions in API specs)
- [ ] GraphQL export (error union types)
- [ ] CLI tool (query errors by code/name)
- [ ] Migration guide generator (auto-doc breaking changes)
- [ ] Error analytics dashboard (track error frequency in production)
- [ ] Cross-contract error linking (document error dependencies)

## Contributing

1. Read `CONTRIBUTING_ERRORS.md`
2. Edit `src/lib.rs` with new error definitions
3. Run `cargo test`
4. Rebuild affected contracts
5. Commit and open a PR

## License

Same as Hermes project.

## Questions?

See:
- `ERROR_DOCS_SYNC.md` — Overall architecture and benefits
- `MIGRATION_ERROR_SYNC.md` — Step-by-step migration guide
- `CONTRIBUTING_ERRORS.md` — Adding/modifying errors
