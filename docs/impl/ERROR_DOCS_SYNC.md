# Error Documentation Sync Solution

## Problem Statement

Three copies of error documentation exist without a sync mechanism, causing inevitable drift:

1. **Enum Definitions** — `contracts/*/src/err.rs` or `errors.rs` (inline Rust code)
2. **Stability Tests** — `contracts/*/tests/err_stab.rs` (manual snapshots)
3. **Comment Headers** — Code range documentation in comments

When one is updated, the others must be manually updated or they diverge. This creates:
- Silent discriminant mismatches
- Inconsistent error codes across contracts
- Failed deployments when tests and enums drift
- Documentation that becomes outdated

## Solution Architecture

A three-layer system ensures a single source of truth with automatic sync:

### Layer 1: Canonical Registry (`crates/hermes-errors/`)

**Location:** `crates/hermes-errors/src/lib.rs`

The canonical error registry defines every error exactly once:

```rust
ErrorDefinition::new(1, "Unauthorized", "Caller is not authorized for the action.")
    .with_range(1, 9),
```

**Guarantees:**
- Every error code defined once
- Numeric codes validated (unique, within range boundaries)
- Scope tracking (which contracts use which errors)
- No manual duplication

### Layer 2: Contract-Specific Generation (Build-time)

When a contract builds, a `build.rs` script:

1. **Reads** the canonical registry
2. **Filters** errors for that contract
3. **Generates** contract-specific files:
   - `src/errors.rs` — Rust enum with inline docs
   - `tests/err_stab.rs` — Stability snapshot tests

**Mechanism:** `crates/hermes-codegen/` provides reusable code generation logic.

```rust
// In contracts/analytics/build.rs
let errors = all_errors()
    .into_iter()
    .filter(|e| e.scope.contains("analytics"))
    .collect();

GenerateErrors::new("analytics", &errors)
    .generate_errors_rs(&out_dir)
    .generate_err_stab_rs(&out_dir);
```

### Layer 3: Compile-time Validation

Tests ensure consistency:

1. **Registry validation** (`crates/hermes-errors/tests/`) — unique codes, range boundaries
2. **Generated file inclusion** — `include!` macro pulls generated files into build
3. **Exhaustive matching** — macro-based snapshots fail to compile if errors are missing

## Migration Path

### Step 1: Add Error to Canonical Registry

Edit `crates/hermes-errors/src/lib.rs`:

```rust
pub fn all_errors() -> Vec<ErrorDefinition> {
    vec![
        // ... existing errors ...
        
        // NEW: Analytics contract
        ErrorDefinition::new(
            114,
            "NewAnalyticsError",
            "Description of the new error."
        )
        .with_range(110, 119),
    ]
}
```

### Step 2: Validate Registry

```bash
cd crates/hermes-errors
cargo test
```

Registry tests ensure:
- No duplicate codes
- All codes within their declared ranges
- All variants documented

### Step 3: Regenerate Contract Files

Run the contract's build script:

```bash
cd contracts/analytics
cargo build
```

This automatically:
1. Generates `src/errors.rs` from the registry
2. Generates `tests/err_stab.rs` from the registry
3. Validates all tests pass

### Step 4: Commit Generated Files (Optional)

Generated files **may be** committed to version control for easier review and to serve as a fallback if the build environment changes. However, they can also be excluded from version control and regenerated on every build.

**Recommendation:** Commit them. This provides:
- Clear git diff showing what changed
- Easier code review (see generated output)
- Reduced build-time generation overhead
- Fallback if build tools are unavailable

Add to `.gitignore` or `.gitkeep` as needed.

## File Structure

```
hermes/
├── crates/
│   ├── hermes-errors/                  # CANONICAL ERROR REGISTRY
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── src/
│   │       └── lib.rs                  # ERROR_CODE_SNAPSHOT macro + validation
│   │
│   └── hermes-codegen/                 # CODE GENERATION LIBRARY
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs                  # GenerateErrors struct
│
└── contracts/
    ├── analytics/
    │   ├── build.rs                    # Calls hermes-codegen to generate files
    │   ├── src/
    │   │   └── errors.rs               # GENERATED: Rust enum
    │   └── tests/
    │       └── err_stab.rs             # GENERATED: Stability snapshot
    │
    ├── allowlist/
    │   ├── build.rs                    # Same pattern
    │   ├── src/
    │   │   └── err.rs                  # GENERATED (or manually updated to use registry)
    │   └── tests/
    │       └── err_stab.rs             # GENERATED
    │
    └── ... (others follow same pattern)
```

## Implementation Checklist

### Phase 1: Foundation (Week 1)

- [x] Create `crates/hermes-errors/` with canonical registry
- [x] Create `crates/hermes-codegen/` with code generation logic
- [x] Add comprehensive documentation and examples
- [ ] Update `Cargo.toml` workspace to include new crates
- [ ] Create build script template for contracts

### Phase 2: Migration (Week 2–3)

Migrate one contract at a time, starting with **analytics** (most complete documentation):

- [ ] Add `contracts/analytics/build.rs` that uses `hermes-codegen`
- [ ] Regenerate `contracts/analytics/src/errors.rs` from registry
- [ ] Regenerate `contracts/analytics/tests/err_stab.rs` from registry
- [ ] Run `cargo test` to validate
- [ ] Repeat for other contracts (allowlist, fees, markets, monitor, resolution, validators)

### Phase 3: Verification & Cleanup (Week 3)

- [ ] Verify all contracts build without errors
- [ ] All tests pass
- [ ] Update CI/CD to regenerate files on every build (optional)
- [ ] Archive old manual error files

### Phase 4: Documentation & Handoff (Week 4)

- [ ] Update contributing guide with new error-addition workflow
- [ ] Document code range allocation strategy
- [ ] Add quick-start guide for adding new errors
- [ ] Train team on new process

## Benefits

| Before | After |
|--------|-------|
| Manual sync of 3 copies | Single source of truth |
| Silent drift possible | Compile-time enforcement |
| Error codes 1–9 per contract | Consistent codes across contracts |
| Time-consuming validation | Automated build-time checking |
| Duplicated stability tests | Generated, always in sync |
| Hard to add new errors | Declarative, repeatable process |

## Maintenance

### Adding a New Error

1. Edit `crates/hermes-errors/src/lib.rs`
2. Run `cargo test` to validate
3. Rebuild affected contracts
4. Commit registry and generated files

### Removing an Error (Deprecation)

1. Mark error as deprecated in registry (add comment)
2. Leave numeric code untouched (never reuse)
3. Rebuild affected contracts
4. Commit changes

### Changing Error Description

1. Edit description in registry
2. Rebuild affected contracts
3. Generated files automatically update
4. Commit changes

## Future Enhancements

1. **OpenAPI/GraphQL Export** — Generate error documentation for client libraries
2. **Error Code Lookup Tool** — CLI to query error by code or name
3. **Migration Guide Generator** — Automate version migration documentation
4. **Cross-contract Error Linking** — Document error dependencies between contracts
5. **Error Analytics** — Track which errors occur most frequently in production

## References

- **Canonical Registry:** `crates/hermes-errors/src/lib.rs`
- **Code Generation:** `crates/hermes-codegen/src/lib.rs`
- **Example Migration:** See `contracts/analytics/build.rs` (after migration)
- **Validation Rules:** `crates/hermes-errors/src/lib.rs` (`validate_*` functions)
