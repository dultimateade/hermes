# CI Integration for Error Validation

This guide covers integrating the error validation test into your CI/CD pipeline.

## GitHub Actions

### Add to your workflow file (e.g., `.github/workflows/test.yml`)

```yaml
- name: Validate error documentation
  run: cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
  if: always()  # Run even if other tests fail
```

### Full example test job:

```yaml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: nightly
          components: rustfmt
          override: true
      
      - name: Build
        run: cargo build
      
      - name: Run tests
        run: cargo test
      
      - name: Validate error documentation
        run: cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
        if: always()
```

## Local Development

### Pre-commit Hook

Create `.git/hooks/pre-commit`:

```bash
#!/bin/bash
# Validate error documentation before committing

echo "Checking error documentation..."
cargo test -p hermes-errors --test validate_contract_errors --quiet

if [ $? -ne 0 ]; then
  cat <<EOF
❌ Error documentation is out of sync with the registry.

Run the following to see details:
  cargo test -p hermes-errors --test validate_contract_errors -- --nocapture

Then either:
1. Add any new errors to crates/hermes-errors/src/lib.rs, or
2. Ensure contract files match the registry

See docs/impl/ERROR_VALIDATION_TEST.md for more information.
EOF
  exit 1
fi

echo "✓ Error documentation is in sync"
exit 0
```

Make it executable:

```bash
chmod +x .git/hooks/pre-commit
```

### Makefile Target

Add to your `Makefile`:

```makefile
.PHONY: validate-errors
validate-errors:
	cargo test -p hermes-errors --test validate_contract_errors -- --nocapture

.PHONY: test
test:
	cargo test
	$(MAKE) validate-errors
```

Usage:

```bash
make validate-errors
```

## CI Failure Handling

### Interpreting Failures

When `validate_contract_errors` fails:

1. **Read the output** — it clearly states what's wrong
2. **Determine the cause**:
   - New error in contract not in registry? → Update `crates/hermes-errors/src/lib.rs`
   - Error code mismatch? → Check contract's discriminant or registry definition
   - Name conflict? → Resolve the name discrepancy

3. **Fix and re-run**:
   ```bash
   cargo test -p hermes-errors --test validate_contract_errors -- --nocapture
   ```

### Common Scenarios

#### Scenario 1: Added a new error to a contract

```rust
// In contracts/analytics/src/errors.rs
pub enum ContractError {
    // ... existing errors ...
    NewFeature = 150,
}
```

**Fix**: Add to registry in `crates/hermes-errors/src/lib.rs`:

```rust
ErrorDefinition::new(150, "NewFeature", "Description of new feature error.")
    .with_range(150, 159),
```

#### Scenario 2: Contract error code was renumbered

```
❌ Code mismatch: analytics contract has 'InvalidConfig' = 31, 
   but registry defines 'InvalidConfig' = 30
```

**Fix**: Ensure the contract's `src/errors.rs` has the code the registry expects, or update the registry if the code change was intentional (⚠ breaking change!).

#### Scenario 3: Unregistered error warning for undeployed errors

```
⚠ Registry has 'FutureError' = 999 but not found in any contract (may be undeployed)
```

This is **not a failure** — it means the registry has an error defined that no contract uses yet. This is normal when:
- Planning ahead for a future deployment
- An error is deprecated but kept for historical tracking

No action needed unless you want to remove the registry entry.

## Monitoring

### Track Error Changes

Add a step to document error changes in PR descriptions:

```bash
# Before running validation, capture the current state
cargo test -p hermes-errors --test validate_contract_errors -- --nocapture > /tmp/error-check.txt 2>&1
```

Then reference this in your PR template:

```markdown
## Error Changes
<!-- Include output from error validation test -->
```

### Dashboard (Future)

Once you have historical error data, you can:
- Track which errors are most frequently hit in production
- Correlate error rates with deployments
- Identify flaky or ambiguous error codes

This data feeds into:
- Error analytics dashboards (in `docs/`)
- Migration guides (in `docs/impl/`)
- Customer-facing error catalogs

## Troubleshooting

### "Cargo not found" in CI

Ensure the CI image has Rust installed. Example for GitHub Actions:

```yaml
- uses: actions-rs/toolchain@v1
  with:
    profile: minimal
    toolchain: nightly  # Match rust-toolchain.toml
    override: true
```

### Test passes locally but fails in CI

Check for:
1. Different Rust versions (use `toolchain: nightly` in CI to match local)
2. Different directory structures in CI (use `CARGO_MANIFEST_DIR` to verify paths)
3. Whitespace or line-ending differences (ensure contracts are checked out with Unix line endings)

### Slow test execution

The test scans all contract files sequentially. To optimize:
- It runs in parallel with other tests via `cargo test` (use `--test-threads=N`)
- No heavy dependencies, so startup is fast
- File I/O is minimal (6 contract files)

Typical runtime: **<100ms**.

## Integration with Code Review

### Pull Request Check

Make this test required for PR merges in your repository settings:

```
Checks Required: test / validate-errors
```

This ensures every PR:
1. Includes registry updates for new errors
2. Doesn't introduce unregistered error codes
3. Maintains the canonical single source of truth

### Review Checklist

When reviewing PRs that modify errors, check:

- [ ] All new error codes added to `crates/hermes-errors/src/lib.rs`
- [ ] Error discriminants match between contract and registry
- [ ] Validation test passes locally
- [ ] Error descriptions are user-facing (not implementation details)
- [ ] Code ranges respect contract allocations (e.g., analytics: 100–199)

## Next Steps

1. **Add the test to your test suite** — use GitHub Actions example above
2. **Set up pre-commit hook** — catch issues before they reach CI
3. **Document error changes** — include in PR templates
4. **Monitor in production** — use error codes for analytics and debugging

For more details, see `docs/impl/ERROR_VALIDATION_TEST.md`.
