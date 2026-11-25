# Error Validation — Quick Start Guide

## TL;DR

Run the validation test to ensure all contract errors are registered:

```bash
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

If it fails, you added/changed a contract error without updating the registry. Fix it:

```bash
# In crates/hermes-errors/src/lib.rs, add the error to all_errors():
ErrorDefinition::new(YOUR_CODE, "YourErrorName", "Description")
    .with_range(YOUR_CODE, MAX_CODE),
```

Then re-run the test.

---

## Common Scenarios

### Scenario 1: Added a New Error to a Contract

**Problem:**
```
You modified contracts/analytics/src/errors.rs and added:
    NewMetricError = 114,
```

**Test Output:**
```
❌ Unregistered error: analytics contract has 'NewMetricError' = 114,
   but this code is not in the registry
```

**Fix:**

1. Open `crates/hermes-errors/src/lib.rs`
2. Find the analytics section (around line 110)
3. Add:
   ```rust
   ErrorDefinition::new(114, "NewMetricError", "The metric computation failed.")
       .with_range(110, 119),
   ```
4. Run the test again:
   ```bash
   cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
   ```

### Scenario 2: Test Passes Locally but Fails in CI

**Possible Causes:**
- Rust version mismatch (use `nightly` in CI)
- Different file paths (check `CARGO_MANIFEST_DIR` calculation)

**Debug:**
```bash
# Run with verbose output
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

### Scenario 3: Error Code Conflict

**Problem:**
```
You set:
  - analytics: InvalidConfig = 131
  - but registry already has: InvalidState = 131
```

**Test Output:**
```
❌ Code mismatch: analytics contract has 'InvalidConfig' = 131,
   but registry defines 'InvalidState' = 131
```

**Fix:**
- Use a different code (e.g., 132, 133, ...)
- Or rename the error to match the registry
- Then re-run the test

### Scenario 4: Pre-commit Hook Blocks Commit

**You see:**
```
Error documentation is out of sync with the registry.
```

**Do this:**
```bash
# Run the test with verbose output to see what's wrong
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture

# Fix the issue (see scenarios above)

# Try the commit again
git commit
```

---

## Integration with Your Workflow

### Option A: Run Before Every Commit

```bash
# Make it part of your test routine
cargo test && cargo test -p hermes-errors --test validate_contract_errors
```

### Option B: Set Up Pre-commit Hook

```bash
# Create .git/hooks/pre-commit
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
cargo test -p hermes-errors --test validate_contract_errors --quiet
EOF

chmod +x .git/hooks/pre-commit
```

### Option C: Add to Makefile

```bash
# Add to Makefile
validate-errors:
	cargo test -p hermes-errors --test validate_contract_errors -- --nocapture

# Run it
make validate-errors
```

### Option D: Let CI Catch It

If you skip local validation, GitHub Actions will catch it:
```yaml
- name: Validate error documentation
  run: cargo test -p hermes-errors --test validate_contract_errors
```

---

## What Gets Validated

| What | Status | Notes |
|------|--------|-------|
| All contract errors registered | ✅ **Required** | Must exist in registry |
| Names match between contract and registry | ✅ **Required** | Same code = same name |
| Codes are unique | ✅ **Required** | No duplicates allowed |
| Registry entries without deployed contracts | ⚠ **Warning only** | OK for future/deprecated errors |

---

## Helpful Commands

```bash
# Run the test (normal)
cargo test -p hermes-errors --test validate_contract_errors

# Run with verbose output (see each match/mismatch)
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture

# Run just the error validation (skip other tests)
cargo test -p hermes-errors validate_contract_errors

# Run all hermes-errors tests
cargo test -p hermes-errors

# Check registry for a specific code
grep "ErrorDefinition::new(101" crates/hermes-errors/src/lib.rs

# View a contract's error definitions
head -50 contracts/analytics/src/errors.rs
```

---

## File Locations

| File | Purpose | Edit When |
|------|---------|-----------|
| `contracts/*/src/errors.rs` | Contract error enums | Adding/changing contract errors |
| `crates/hermes-errors/src/lib.rs` | **Registry** (edit this!) | Adding/changing registered errors |
| `crates/hermes-errors/tests/validate_contract_errors.rs` | The validation test | Understanding how validation works |

---

## Troubleshooting

### "Unregistered error" but I added it to the registry

**Check:**
1. Spelling: does the registry code match the contract? (e.g., 114 vs 141)
2. Name: does the registry name match exactly? (case-sensitive)
3. Syntax: run `cargo build -p hermes-errors` to check for Rust errors

**Fix:**
```bash
# Double-check the registry
cargo test -p hermes-errors

# If the registry compiles, run the validation test again
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

### Test can't find contract files

This shouldn't happen, but if it does:

```bash
# Check that contracts exist
ls -la contracts/*/src/errors.rs

# Run from the project root
cd /workspaces/hermes
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
```

### "Code mismatch" between contract and registry

**This is bad!** It means:
- Contract deployed with code `X`
- Registry expects code `Y`
- **Off-chain systems expecting code `X` will break!**

**Solution:**
1. **Do NOT renumber deployed codes** (breaking change)
2. Instead, update the registry to match the contract
3. Document the change in your PR description

```bash
# Example: if contract has code 114 but registry has 115
# Fix the registry:
ErrorDefinition::new(114, "MyError", "...")  # ← Change 115 to 114
```

---

## See Also

- **Full Test Guide**: `docs/impl/ERROR_VALIDATION_TEST.md`
- **Architecture Deep Dive**: `docs/impl/ERROR_VALIDATION_ARCHITECTURE.md`
- **CI Integration**: `docs/impl/CI_ERROR_VALIDATION_INTEGRATION.md`
- **Error Registry**: `crates/hermes-errors/README.md`
- **Contributing Errors**: `docs/impl/CONTRIBUTING_ERRORS.md`

---

## Still Stuck?

1. Read the test output carefully — it tells you exactly what's wrong
2. Check the file locations table above
3. Compare contract error code to registry entry code (must match exactly)
4. Run `cargo test -p hermes-errors --test validate_contract_errors -- --nocapture` for verbose output
5. Reach out to the team if you think the test is wrong (it probably isn't, but let's check!)

**Remember**: The test is your friend. It catches errors before they reach production. ✅
