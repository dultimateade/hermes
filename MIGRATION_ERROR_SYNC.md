# Migration Guide: Error Documentation Sync

This guide walks through migrating a single contract from manual error definitions to the canonical error registry system.

## Prerequisites

- Canonical registry created: `crates/hermes-errors/`
- Code generation library created: `crates/hermes-codegen/`
- Workspace `Cargo.toml` updated to include both crates

## Quick Start (5 minutes)

### For Analytics Contract

1. **Add dependency to `contracts/analytics/Cargo.toml`:**

```toml
[package]
name = "analytics"
# ... other metadata ...

[dependencies]
hermes-errors = { path = "../../crates/hermes-errors" }
hermes-codegen = { path = "../../crates/hermes-codegen" }
# ... other dependencies ...
```

2. **Create `contracts/analytics/build.rs`:**

```rust
use hermes_codegen::GenerateErrors;
use hermes_errors::all_errors;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    println!("cargo:rerun-if-changed=../../crates/hermes-errors/src/lib.rs");

    let analytics_errors: Vec<_> = all_errors()
        .into_iter()
        .filter(|e| e.code == 1 || (e.code >= 100 && e.code < 200))
        .collect();

    GenerateErrors::new("analytics", &analytics_errors)
        .generate_errors_rs(&out_dir);
}
```

3. **Verify canonical registry is current:**

```bash
cd crates/hermes-errors
cargo test
# Verify all tests pass
```

4. **Build the contract:**

```bash
cd contracts/analytics
cargo build
```

Generated `src/errors.rs` appears in the build output directory.

5. **Compare with existing error definitions:**

```bash
# Generate the new file
cargo build

# Check what changed
diff src/errors.rs $(cargo metadata --format-version 1 | jq -r '.target_directory')/errors.rs
```

6. **If satisfied, move generated file to source:**

```bash
# Linux/Mac
cp target/debug/build/analytics-*/out/errors.rs src/errors.rs

# PowerShell (Windows)
Copy-Item -Path "target\debug\build\analytics-*\out\errors.rs" -Destination "src\errors.rs"
```

7. **Update code to use the new file:**

If the generated `src/errors.rs` replaces an older `src/err.rs` or inline definitions, update imports:

```rust
// Before
pub mod err;
use err::AllowlistError;

// After
pub mod errors;  // Use generated file
use errors::ContractError;
```

8. **Run tests:**

```bash
cd contracts/analytics
cargo test
```

## Step-by-Step Detailed Migration

### Phase 1: Prepare the Registry

**Before starting any contract migration, ensure:**

1. All current error definitions from the contract are in the canonical registry
2. Registry tests pass

**Action:**

```bash
cd crates/hermes-errors
cargo test
```

If tests fail:
- Check `src/lib.rs` for duplicate codes
- Review range boundaries
- Fix issues before proceeding

### Phase 2: Verify Registry Accuracy

**Manually verify the registry contains all current errors for the contract:**

1. Open `contracts/analytics/src/errors.rs` (current)
2. Open `crates/hermes-errors/src/lib.rs` (registry)
3. Checklist:
   - [ ] Every variant in analytics has a corresponding definition
   - [ ] Code numbers match exactly
   - [ ] Descriptions are accurate
   - [ ] Code ranges are correct

**Example:**

Current analytics errors:
```rust
pub enum ContractError {
    Unauthorized = 1,
    AdminNotSet = 2,
    NotInitialized = 3,
    // ...
}
```

Registry should have:
```rust
ErrorDefinition::new(101, "Unauthorized", "..."),
ErrorDefinition::new(102, "AdminNotSet", "..."),
ErrorDefinition::new(103, "NotInitialized", "..."),
// ...
```

### Phase 3: Set Up Build Script

**Add `contracts/analytics/build.rs`:**

```rust
use hermes_codegen::GenerateErrors;
use hermes_errors::all_errors;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    
    // Notify Cargo to rerun if registry changes
    println!("cargo:rerun-if-changed=../../crates/hermes-errors/src/lib.rs");
    
    // Get all errors
    let all = all_errors();
    
    // Filter for this contract (example for analytics)
    let contract_errors: Vec<_> = all
        .into_iter()
        .filter(|e| {
            // Universal error: Unauthorized
            e.code == 1 ||
            // Analytics-specific block: 100-199
            (e.code >= 100 && e.code < 200)
        })
        .collect();
    
    // Generate files
    GenerateErrors::new("analytics", &contract_errors)
        .generate_errors_rs(&out_dir);
}
```

**Update `contracts/analytics/Cargo.toml`:**

```toml
[dependencies]
hermes-errors = { path = "../../crates/hermes-errors" }
hermes-codegen = { path = "../../crates/hermes-codegen" }
# ... keep existing dependencies ...
```

### Phase 4: Generate Files

**Build the contract:**

```bash
cd contracts/analytics
cargo build
```

**What happens:**
1. `build.rs` reads the canonical registry
2. Filters errors for analytics (codes 1, 100-199)
3. Generates `target/debug/build/analytics-*/out/errors.rs`

**Inspect generated file:**

```bash
# On Windows PowerShell
$out = cargo metadata --format-version 1 | ConvertFrom-Json | Select-Object -ExpandProperty target_directory
Get-Content "$out\debug\build\analytics-*\out\errors.rs" | head -50

# On Linux/Mac
OUT=$(cargo metadata --format-version 1 | jq -r '.target_directory')
head -50 "$OUT/debug/build/analytics-*/out/errors.rs"
```

### Phase 5: Validate Generated Content

**Verify the generated file includes:**

- [ ] Correct module header with generation warning
- [ ] All expected enum variants
- [ ] Correct discriminant values
- [ ] Inline documentation for each error

**Example inspection:**

```rust
//! Error types for the analytics contract.
//!
//! This file is **generated** from the canonical error registry...

#[contracterror]
pub enum ContractError {
    /// Caller is not authorized...
    Unauthorized = 1,
    /// Admin address has not been set...
    AdminNotSet = 102,
    // ...
}
```

### Phase 6: Integration

**Option A: Commit generated files**

Good for:
- Code review (see diffs)
- Fallback if build tools unavailable
- Slightly faster builds

```bash
# Copy generated file
cp target/debug/build/analytics-*/out/errors.rs src/errors.rs

# Stage and commit
git add src/errors.rs build.rs Cargo.toml
git commit -m "feat: migrate analytics errors to canonical registry

- Add build.rs to generate errors.rs from hermes-errors registry
- Update analytics errors to use codes 100-199 (analytics block)
- Ensures error discriminants stay in sync across contracts"
```

**Option B: Exclude generated files from version control**

Good for:
- Reduced git history churn
- Always fresh generation
- Simpler workflow

```bash
# Add to .gitignore
echo "src/errors.rs" >> contracts/analytics/.gitignore

# Regenerate on build
cargo build
```

### Phase 7: Testing

**Run contract tests:**

```bash
cd contracts/analytics
cargo test
```

**Expected results:**
- All existing tests pass
- No new test failures
- Error codes match registry

**If tests fail:**

Common issues:

| Issue | Solution |
|-------|----------|
| Import errors | Update `use` statements to match new enum names |
| Discriminant mismatch | Check registry codes; sync may have renumbered for safety |
| Missing variants | Verify all errors are in the registry for this contract |

### Phase 8: Update Stability Tests

**If contract has `tests/err_stab.rs`:**

Option 1: Generate it (if `hermes-codegen` supports it)

```rust
GenerateErrors::new("analytics", &contract_errors)
    .generate_errors_rs(&out_dir)
    .generate_err_stab_rs(&out_dir);  // Also generate tests
```

Then move generated test file:

```bash
cp target/debug/build/analytics-*/out/err_stab.rs tests/err_stab.rs
```

Option 2: Manually update it

```rust
// tests/err_stab.rs
#[test]
fn analytics_error_codes_are_stable() {
    let expected_codes = [
        (ContractError::Unauthorized, 1),
        (ContractError::AdminNotSet, 102),
        (ContractError::NotInitialized, 103),
        // ... all errors
    ];
    
    for (error, code) in &expected_codes {
        assert_eq!(*error as u32, *code);
    }
}
```

### Phase 9: Documentation Update

**Update contract README if needed:**

```markdown
## Error Codes

Error definitions are generated from the canonical registry in `crates/hermes-errors/`.

To add a new error:

1. Edit `crates/hermes-errors/src/lib.rs`
2. Run `cargo build` in this contract
3. Commit the generated `src/errors.rs`

See `../../ERROR_DOCS_SYNC.md` for details.
```

## Repeat for Other Contracts

Once analytics is migrated successfully, repeat the process for:

1. **allowlist** — errors 1–99 (primarily 1–10)
2. **fees** — errors 200–299
3. **markets** — errors 300–399
4. **monitor** — errors 400–499
5. **resolution** — errors 500–599
6. **validators** — errors 600–699

Each follows the same pattern:
- Add build.rs with appropriate code range filter
- Verify registry contains all errors
- Generate, validate, and integrate

## Troubleshooting

### Build fails: "hermes-errors not found"

**Solution:** Update workspace `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/hermes-errors",
  "crates/hermes-codegen",
  "contracts/*",
  # ...
]
```

Then run:

```bash
cargo build
```

### Generated file has different codes than current

**This is expected!** The registry may assign codes differently to avoid collisions.

**Action:**

1. Review code changes
2. Verify they match the registry design
3. Update any contract code that depends on specific codes
4. Test thoroughly
5. Document migration notes in PR

### Registry validation fails

**Solution:**

```bash
cd crates/hermes-errors
cargo test
```

Output will show:
- Duplicate codes
- Out-of-range codes
- Other validation errors

Fix issues in `src/lib.rs` and rerun tests.

### Tests fail after migration

**Checklist:**

- [ ] Error discriminants match registry
- [ ] Import paths are correct
- [ ] All enum variants are defined
- [ ] Code that pattern-matches on errors is updated

Debug by:

```bash
# View all errors in contract
grep -n "ContractError::" src/*.rs tests/*.rs

# Compare with registry
cd ../../crates/hermes-errors
grep -n "ErrorDefinition::new" src/lib.rs
```

## Rollback

If migration fails and needs to be reverted:

```bash
# Revert build.rs addition
git checkout HEAD -- contracts/analytics/build.rs

# Restore original error file
git checkout HEAD -- contracts/analytics/src/errors.rs

# Revert Cargo.toml
git checkout HEAD -- contracts/analytics/Cargo.toml

# Clean build artifacts
cargo clean

# Rebuild
cargo build
```

## Success Criteria

✅ Migration is successful when:

- [ ] Contract builds without errors: `cargo build` passes
- [ ] All tests pass: `cargo test` passes
- [ ] Error codes match registry: Compare generated file with registry
- [ ] No manual error file edits: Only `build.rs` and `Cargo.toml` changed
- [ ] Documentation updated: README or error handling guide updated
- [ ] PR review approved: Team agrees on approach and implementation

## Questions?

See main documentation:
- **Overview:** `ERROR_DOCS_SYNC.md`
- **Registry:** `crates/hermes-errors/src/lib.rs`
- **Code gen:** `crates/hermes-codegen/src/lib.rs`
