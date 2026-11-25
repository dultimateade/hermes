# Contributing: Adding and Modifying Errors

Quick reference for adding, modifying, or removing errors in the Hermes system.

## TL;DR

To add a new error:

1. Open `crates/hermes-errors/src/lib.rs`
2. Add `ErrorDefinition::new(CODE, "Name", "Description")` to `all_errors()`
3. Run `cargo test -p hermes-errors` (validate unique codes)
4. Rebuild affected contracts: `cargo build -p contracts/<name>`
5. Commit registry and regenerated error files

## Adding a New Error

### Step 1: Choose an Error Code

**Code ranges** are reserved per contract:

| Contract   | Block    | Range     | Purpose             |
|------------|----------|-----------|---------------------|
| allowlist  | 0–99     | 1–9       | auth                |
| analytics  | 100–199  | 101–109   | auth                |
|            |          | 110–119   | data / query        |
|            |          | 120–129   | metric / aggregation|
|            |          | 130–139   | config / admin      |
| fees       | 200–299  | 201–209   | auth                |
| markets    | 300–399  | 301–309   | auth                |
| monitor    | 400–499  | 401–409   | auth                |
| resolution | 500–599  | 501–509   | auth                |
| validators | 600–699  | 601–609   | auth                |

**Guidelines:**

- Use the first **unused** code within your contract's reserved block
- If adding a related error, use the next code in the same range (e.g., 102, 103, etc.)
- Never reuse a code that was previously used, even if that error was removed

### Step 2: Edit the Registry

Open `crates/hermes-errors/src/lib.rs` and add to `all_errors()`:

```rust
pub fn all_errors() -> Vec<ErrorDefinition> {
    vec![
        // ... existing errors ...
        
        // NEW ERROR
        ErrorDefinition::new(
            114,  // Code: next unused in 110-119 range
            "InvalidMetricType",  // Name
            "The requested metric type is not supported by this contract."  // Description
        )
        .with_range(110, 119),  // Belongs to data/query range
    ]
}
```

### Step 3: Validate the Registry

```bash
cd crates/hermes-errors
cargo test
```

**All tests must pass:**

- `registry_has_no_duplicate_codes` — code uniqueness
- `registry_respects_range_boundaries` — codes in correct ranges
- Other validation tests

**If tests fail:**

The error message tells you what's wrong:

| Error | Action |
|-------|--------|
| `Duplicate error code: 114` | Choose a different code |
| `Error InvalidMetricType falls outside its declared range (110-119)` | Fix the range or code |
| Other validation errors | Follow the message guidance |

### Step 4: Rebuild Affected Contracts

The error is added to the registry. Now regenerate contract files:

```bash
# Single contract
cd contracts/analytics
cargo build

# All contracts
cargo build
```

**What happens:**

1. Contract's `build.rs` reads the updated registry
2. Generates new `src/errors.rs` with your new error
3. Optionally generates new `tests/err_stab.rs`

### Step 5: Verify Generated Files

Check that the generated file includes your new error:

```bash
# On Windows PowerShell
$out = cargo metadata --format-version 1 | ConvertFrom-Json | Select-Object -ExpandProperty target_directory
Select-String "InvalidMetricType" "$out\debug\build\analytics-*\out\errors.rs"

# On Linux/Mac
grep "InvalidMetricType" target/debug/build/analytics-*/out/errors.rs
```

### Step 6: Use the New Error in Code

Update contract code to return the new error:

```rust
// In contracts/analytics/src/lib.rs

use crate::errors::ContractError;

pub fn analyze_metric(metric_type: &str) -> Result<Value, ContractError> {
    match metric_type {
        "volume" | "price" => Ok(compute_metric(metric_type)),
        _ => Err(ContractError::InvalidMetricType),  // NEW ERROR
    }
}
```

### Step 7: Update Tests (if applicable)

If you created a stability test file, update it:

```rust
// contracts/analytics/tests/err_stab.rs

#[test]
fn analytics_error_codes_are_stable() {
    let codes = vec![
        (ContractError::InvalidMetricType, 114),
        // ... all other errors ...
    ];
    
    for (error, expected) in codes {
        assert_eq!(error as u32, expected);
    }
}
```

### Step 8: Test Thoroughly

```bash
cd contracts/analytics
cargo test

# Also test that the new error is actually returned
cargo test analyze_metric
```

### Step 9: Commit

```bash
git add -A
git commit -m "feat(analytics): add InvalidMetricType error (code 114)

- Add error definition to hermes-errors registry
- Regenerate analytics error files
- Includes new error code, range validation, stability tests"
```

## Modifying an Existing Error

### Changing Error Description

**This is safe and does NOT require versioning:**

1. Edit description in `crates/hermes-errors/src/lib.rs`
2. Run tests: `cargo test -p hermes-errors`
3. Rebuild contracts: `cargo build`
4. Commit: `git commit -m "docs(errors): clarify description of error XYZ"`

**Why it's safe:**
- Client code doesn't depend on documentation
- Discriminant (code) is unchanged
- Only affects display/logging

### Changing Error Code

**This is BREAKING and requires migration planning:**

⚠️ **Do NOT do this** after the error code is deployed to production.

For pre-release changes only:

1. Update code in registry
2. Document the change in PR description: "Migration: Error XYZ moved from 100 → 101"
3. Include migration steps in changelog
4. Alert users and give them time to update

**Never** renumber after deployment:
- Breaks client code that pattern-matches on codes
- Breaks off-chain analytics dashboards
- Breaks monitoring systems

### Renaming an Error Variant

**This is safe but may affect clients:**

1. Update the `name` field in registry
2. Old code consuming the enum might break at compile time
3. This is intentional: force users to update references

Example:

```rust
// Before
ErrorDefinition::new(110, "MarketNotFound", "...")

// After
ErrorDefinition::new(110, "QueryMarketNotFound", "...")  // Clarified name
```

Compile-time breakage ensures all usages are found and updated.

## Removing an Error (Deprecation)

**Never delete an error code.** Instead, deprecate it:

### Option 1: Mark as Deprecated

Add a deprecation comment:

```rust
ErrorDefinition::new(
    42,
    "ObsoleteErrorCode",
    "DEPRECATED as of v1.2.0. Use XYZ instead. This code is reserved and will never be reused."
)
.with_range(1, 99),
```

Rebuild contracts. Clients can still receive the error code, but:
- Documentation warns them it's deprecated
- New code should not return this error

### Option 2: Explicitly Retire (for future versions)

If you absolutely must retire a code:

1. Document it in `CHANGELOG.md` as a breaking change
2. Remove it from the registry
3. Update stability tests
4. Require clients to upgrade
5. Note: the code number can never be reused

**Best practice:** Use Option 1 (deprecation) when possible.

## Special Cases

### Cross-Contract Errors

Some errors appear in multiple contracts (e.g., `Unauthorized`).

**Pattern:**

```rust
// UNIVERSAL ERROR: Unauthorized (1)
// Used by: allowlist, analytics, fees, markets, monitor, resolution, validators
ErrorDefinition::new(1, "Unauthorized", "Caller is not authorized for the action."),

// CONTRACT-SPECIFIC: analytics version
ErrorDefinition::new(101, "Unauthorized", "Caller is not authorized to perform the requested action."),
```

Each contract can have its own version with slightly different documentation.

### Related Errors

Errors in the same range often relate to each other:

```rust
// Data/Query range (110-119)
ErrorDefinition::new(110, "MarketNotFound", "..."),
ErrorDefinition::new(111, "SnapshotNotFound", "..."),
ErrorDefinition::new(112, "InvalidTimeRange", "..."),
```

When adding related errors, use consecutive codes within the range.

## Validation Rules

The registry enforces:

1. **Unique codes** — no two errors share a code
2. **Range boundaries** — codes respect their declared ranges
3. **Non-overlapping ranges** — ranges don't collide across contracts
4. **Non-reusable codes** — once used, never reused

Run validation:

```bash
cd crates/hermes-errors
cargo test validate
```

## Code Review Checklist

When reviewing error-related PRs:

- [ ] New error added to `crates/hermes-errors/src/lib.rs`
- [ ] Registry tests pass: `cargo test -p hermes-errors`
- [ ] Code is unique (not a duplicate)
- [ ] Code is within the correct range
- [ ] Description is clear and user-facing
- [ ] Contracts are rebuilt and files regenerated
- [ ] Contract tests pass: `cargo test`
- [ ] No manual edit to generated `errors.rs` files
- [ ] PR description notes if this is a breaking change

## Examples

### Example 1: Add a new validation error to fees

```rust
// In crates/hermes-errors/src/lib.rs

// Fees contract (200-299 block)
ErrorDefinition::new(
    205,
    "InvalidFeePercentage",
    "The provided fee percentage is outside the valid range (0-100)."
)
.with_range(205, 209),  // Validation range
```

Then:

```bash
cd crates/fees
cargo build
cargo test
```

### Example 2: Add multiple related errors

```rust
// In crates/hermes-errors/src/lib.rs

// Markets contract (300-399 block), resolution range (320-329)
ErrorDefinition::new(320, "ResolutionTooEarly", "Resolution cannot proceed; market is still active."),
ErrorDefinition::new(321, "ResolutionTooLate", "Resolution period has expired; outcomes are final."),
ErrorDefinition::new(322, "ResolutionConflict", "Conflicting resolution attempts detected; escalation required."),
```

### Example 3: Deprecate an error

```rust
// In crates/hermes-errors/src/lib.rs

ErrorDefinition::new(
    88,
    "LegacyFormatError",
    "DEPRECATED as of v2.0.0. Use InvalidInput instead. Reserved code, will never be reused."
)
.with_range(1, 99),
```

## Frequently Asked Questions

**Q: Can I use code 99 in multiple contracts?**

A: No. Code ranges are global and non-overlapping. Allowlist reserves 1-99, so no other contract can use those codes.

**Q: What if I run out of codes in my range?**

A: Expand the range if there are unused codes later, or request additional codes from the maintainers. Plan ahead.

**Q: Can I reorder errors in the enum?**

A: Yes, as long as discriminants don't change. The numeric code is what matters, not the enum order.

**Q: Do I need to update tests when I modify a description?**

A: Only if tests check the description text. Most tests only check discriminants, which don't change.

**Q: How do I know if my error code will conflict with another contract?**

A: The registry validates this automatically: `cargo test -p hermes-errors`.

## Getting Help

- **Registry format:** See `crates/hermes-errors/src/lib.rs`
- **Code generation:** See `crates/hermes-codegen/src/lib.rs`
- **Full migration:** See `ERROR_DOCS_SYNC.md` and `MIGRATION_ERROR_SYNC.md`
- **Questions:** Open an issue or ask in team chat
