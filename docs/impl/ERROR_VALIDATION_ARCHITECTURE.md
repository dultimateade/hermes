# Error Validation Architecture & Data Flow

## System Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                       HERMES ERROR VALIDATION                      │
│                         Integration Test                           │
└────────────────────────────────────────────────────────────────────┘

     ┌──────────────────────┐         ┌─────────────────────────┐
     │ Contract Error Files │         │ Canonical Registry      │
     │                      │         │                         │
     │ • analytics          │         │ crates/hermes-errors/   │
     │ • fees               │         │   src/lib.rs            │
     │ • markets            │         │                         │
     │ • monitor            │         │ ErrorDefinition {       │
     │ • resolution         │         │   code: u32,            │
     │ • validators         │         │   name: &str,           │
     │                      │         │   description: &str,    │
     │ Format:              │         │   range: Option<(u,u)>  │
     │ #[contracterror]     │         │ }                       │
     │ pub enum            │         │                         │
     │ ContractError {      │         │ Validation functions:   │
     │   Error1 = 1,        │         │ • validate_unique_codes │
     │   Error2 = 10,       │         │ • validate_range_bounds │
     │   ...                │         │ • find_by_code          │
     │ }                    │         │ • find_by_name          │
     └──────────────────────┘         └─────────────────────────┘
            ▲                                      ▲
            │                                      │
            │ reads & parses                       │ queries
            │                                      │
            └──────────────┬───────────────────────┘
                           │
                    ┌──────▼────────┐
                    │   Integration │
                    │     Test      │
                    │               │
                    │ validate_     │
                    │ contract_     │
                    │ errors.rs     │
                    └───────────────┘
                           │
                           │ output: pass/fail + detailed report
                           ▼
                    ┌──────────────┐
                    │ CI Pipeline  │
                    │ Pre-commit   │
                    │ Local dev    │
                    └──────────────┘
```

## Test Execution Flow

```
1. START: cargo test validate_contract_errors
                     │
                     ▼
2. SCAN CONTRACTS
   └─ Locate all contracts/*/src/errors.rs files
      ├─ contracts/analytics/src/errors.rs ✓ found
      ├─ contracts/fees/src/errors.rs ✓ found
      ├─ contracts/markets/src/errors.rs ✓ found
      ├─ contracts/monitor/src/errors.rs ✓ found
      ├─ contracts/resolution/src/errors.rs ✓ found
      └─ contracts/validators/src/errors.rs ✓ found
                     │
                     ▼
3. EXTRACT ERRORS (per file)
   ├─ Parse line by line
   ├─ Skip comments/decorators
   ├─ Match: "VariantName = code,"
   └─ Extract: (name, code, contract_name)
                     │
                     ▼
4. COLLECT RESULTS
   ├─ All errors: Vec<ExtractedError>
   └─ Example: ExtractedError {
        name: "Unauthorized",
        code: 101,
        contract: "analytics",
        source_file: ".../contracts/analytics/src/errors.rs"
      }
                     │
                     ▼
5. LOAD REGISTRY
   └─ Call all_errors() from hermes-errors/src/lib.rs
      └─ Returns: Vec<ErrorDefinition>
                     │
                     ▼
6. COMPARE (for each extracted error)
   │
   ├─ Lookup by code: find_by_code(code)
   │
   ├─ If found:
   │  ├─ Compare names
   │  ├─ Name matches? → ✓ Pass
   │  └─ Name mismatch? → ❌ Fail
   │
   └─ If not found:
      └─ ❌ Fail (unregistered error)
                     │
                     ▼
7. CHECK REGISTRY COMPLETENESS
   └─ For each registry entry code > 100:
      └─ Found in any contract? 
         ├─ Yes → OK (deployed)
         └─ No → ⚠ Warn (undeployed)
                     │
                     ▼
8. REPORT RESULTS
   ├─ Print: extracted count, registry count
   ├─ Print: each match (✓), mismatch (❌), warning (⚠)
   │
   ├─ If mismatches exist:
   │  └─ Print each mismatch detail
   │  └─ Panic with "Error definitions do not match registry!"
   │
   └─ If all match:
      └─ Print "✓ All contract errors match the registry!"
                     │
                     ▼
9. EXIT
   └─ Status: pass (0) or fail (101)
```

## Data Flow: Single Error Through the System

```
┌─────────────────────────────────────────────────────────────────────┐
│ Example: Tracking "Unauthorized" error code 101 through the system  │
└─────────────────────────────────────────────────────────────────────┘

STEP 1: Source File (contracts/analytics/src/errors.rs)
┌──────────────────────────────────┐
│  #[contracterror]                │
│  pub enum ContractError {         │
│      Unauthorized = 101,    ←─────┼─── source code defines code 101
│      AdminNotSet = 102,           │
│  }                                │
└──────────────────────────────────┘
           │
           │ line-based parser
           ▼
STEP 2: Extracted Error
┌──────────────────────────────────┐
│  ExtractedError {                │
│    name: "Unauthorized",          │
│    code: 101,                     │
│    contract: "analytics",         │
│    source_file: "...errors.rs"    │
│  }                                │
└──────────────────────────────────┘
           │
           │ lookup by code
           ▼
STEP 3: Registry Lookup
┌──────────────────────────────────┐
│  find_by_code(101) → Some({       │
│    code: 101,                     │
│    name: "Unauthorized",          │
│    description: "Caller is not..."│
│    range: Some((101, 109)),       │
│    scope: { "analytics" }         │
│  })                               │
└──────────────────────────────────┘
           │
           │ compare names
           ▼
STEP 4: Validation Result
┌──────────────────────────────────┐
│  "Unauthorized" == "Unauthorized"?│
│  YES ✓                            │
│                                   │
│  Result: Match                    │
│  Output: "✓ analytics contract:   │
│           Unauthorized = 101      │
│           (matches registry)"     │
└──────────────────────────────────┘
           │
           │ accumulate result
           ▼
STEP 5: Test Report
┌──────────────────────────────────┐
│  matches: [✓ Unauthorized]        │
│  mismatches: []                   │
│  warnings: []                     │
│  test_passed: true                │
└──────────────────────────────────┘
```

## Error Path: Unregistered Error Detection

```
Contract Code:                      Registry:
┌──────────────────────┐           ┌──────────────────────┐
│ NewError = 150       │  lookup   │ (doesn't contain 150)│
└──────────────────────┘           └──────────────────────┘
           │                                 │
           └──────────────┬──────────────────┘
                          │
                    find_by_code(150)
                          │
                          ▼
                      Option::None
                          │
            ┌─────────────────────────┐
            ▼                         ▼
        Option::Some               Option::None
     (expected path)            (error path)
                                    │
                                    ▼
                            Fail: "Unregistered error:
                            [contract] has '[Name]' = [code],
                            but this code is not in the registry"
                                    │
                                    ▼
                            test_panic!()
```

## CI Integration Point

```
┌────────────────────────────────────────────┐
│  Developer Commits                         │
│  ├─ updates contracts/*/src/errors.rs      │
│  └─ (forgets to update registry)           │
└────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────┐
│  Pre-commit Hook (optional, local)         │
│  run: validate_contract_errors             │
│  ├─ Detects mismatch ❌                    │
│  └─ blocks commit                          │
└────────────────────────────────────────────┘
             │ (if bypassed)
             ▼
┌────────────────────────────────────────────┐
│  GitHub Push to Branch                     │
└────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────┐
│  CI Pipeline (GitHub Actions)              │
│  ├─ run: cargo test                        │
│  ├─ run: validate_contract_errors ✓ key   │
│  └─ if fails: block PR merge               │
└────────────────────────────────────────────┘
             │
             ▼ (fails)
┌────────────────────────────────────────────┐
│  PR Feedback                               │
│  ❌ Error documentation validation failed  │
│  "Unregistered error: [details]"           │
│  "See: docs/impl/ERROR_VALIDATION_TEST.md" │
└────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────┐
│  Developer Fixes (requires action)         │
│  ├─ update crates/hermes-errors/src/lib.rs│
│  ├─ add ErrorDefinition for new code      │
│  └─ commit + push                          │
└────────────────────────────────────────────┘
             │
             ▼
┌────────────────────────────────────────────┐
│  Re-run CI                                 │
│  ✓ validate_contract_errors passes         │
│  ✓ PR approved                             │
└────────────────────────────────────────────┘
```

## Test Output States

```
┌─────────────────────────────────────────────────────┐
│           POSSIBLE TEST OUTCOMES                    │
└─────────────────────────────────────────────────────┘

STATE 1: ✓ PASS
┌──────────────────────┐
│ All errors match     │
│ All codes unique     │
│ No registry drift    │
│                      │
│ exit(0) - success    │
└──────────────────────┘
       example:
    ✓ analytics: Unauthorized = 101
    ✓ fees: FeesPaused = 6
    ✓ all contract errors match

STATE 2: ❌ FAIL - Unregistered Error
┌──────────────────────┐
│ Contract has code    │
│ Registry missing it  │
│                      │
│ exit(101) - failure  │
└──────────────────────┘
       example:
    ❌ Unregistered error: fees contract
       has 'NewFeeType' = 20, but this
       code is not in the registry

STATE 3: ❌ FAIL - Code Mismatch
┌──────────────────────┐
│ Same code, diff name │
│ Registry conflict    │
│                      │
│ exit(101) - failure  │
└──────────────────────┘
       example:
    ❌ Code mismatch: analytics contract
       has 'InvalidConfig' = 131, but
       registry defines 'InvalidState' = 131

STATE 4: ⚠ WARN - Undeployed (info only)
┌──────────────────────┐
│ Registry has code    │
│ No contract uses it  │
│                      │
│ exit(0) - continue   │
│ (not a failure)      │
└──────────────────────┘
       example:
    ⚠ Registry has 'FutureError' = 999
      but not found in any contract
      (may be undeployed)
```

## Performance Profile

```
Operation                Time        Notes
──────────────────────────────────────────────────────
Startup                  <10ms       Minimal deps
Scan 6 contracts         ~20ms       6 × read_dir + open
Parse errors             ~30ms       line-by-line parsing
Load registry             ~20ms       BTreeMap lookups
Comparison               ~10ms       O(n) lookups
Report generation        <5ms        print output
──────────────────────────────────────────────────────
Total (typical)          ~95ms       <100ms in most cases
Memory usage             ~1MB        Extracted + registry
```

## Integration Points

### Local Development

```
Developer Workflow:

1. modify contracts/*/src/errors.rs
   ↓
2. run: cargo build
   ↓
3. run: cargo test -p hermes-errors --test validate_contract_errors
   ├─ ✓ pass? commit
   └─ ❌ fail? update registry and retry
   ↓
4. git push
```

### CI/CD Pipeline

```
GitHub Actions:

push → checkout → install rust → cargo test
                                     ├─ all tests
                                     └─ validate_contract_errors (required)
                                            ├─ ✓ pass? merge allowed
                                            └─ ❌ fail? block merge
```

### Pre-commit Hook

```
Local Git:

git commit → pre-commit hooks → validate_contract_errors
                                    ├─ ✓ pass? proceed
                                    └─ ❌ fail? abort commit
```

## Error Resolution Flowchart

```
             Test Fails?
                 │
    ┌────────────┼────────────┐
    │            │            │
    ▼            ▼            ▼
Unregistered  Mismatch   Undeployed
  Error        Error      Warning
    │            │            │
    │            │        No action
    │            │        (info only)
    │            │
    └──┬─────────┘
       │
       ▼
   READ ERROR MESSAGE
       │
       ├─ "Unregistered error: [contract]
       │   has '[Name]' = [code]"
       │   │
       │   ├─ New error in contract
       │   └─ Action: Add to registry
       │
       └─ "Code mismatch: [contract]
          has '[Name1]' = [code],
          registry defines '[Name2]' = [code]"
          │
          ├─ Code conflict
          └─ Action: Align codes or names
               │
               ▼
           FIX IT
               │
               ├─ Edit crates/hermes-errors/src/lib.rs
               ├─ Add/update ErrorDefinition
               ├─ commit
               └─ cargo test
                      │
                      ├─ ✓ pass
                      └─ ❌ still fail? re-read error
```

## Summary

The validation system works as a **continuous sync mechanism** between:

1. **Contract implementations** — actual error codes deployed
2. **Canonical registry** — single source of truth
3. **CI/CD pipeline** — enforcement gate

This creates a **tight feedback loop** where:
- New errors are caught immediately
- Drift is impossible to ship
- Documentation stays authoritative
- Deployment confidence increases

```
┌──────────────────────────────────────────────┐
│  Error Registration Cycle                    │
│                                              │
│  1. Dev adds error to contract               │
│  2. Test detects unregistered code           │
│  3. Dev updates registry                     │
│  4. Test validates match                     │
│  5. Deployment proceeds with confidence      │
│                                              │
│  No drift possible → registry always current │
└──────────────────────────────────────────────┘
```
