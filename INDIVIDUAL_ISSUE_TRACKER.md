# Individual Issue Tracker

This document breaks down the blockers previously documented in PR_NOTES.md into individual tracked issues that can be assigned, prioritized, and closed when resolved.

## Resolved Issues (from PR #xxx - fix/issues-103-104-105-106)

### Issue: Symbol length violations (Symbol::new > 9 char limit)
**Status**: ✅ RESOLVED  
**Description**: `Symbol::new(env, "max_bet_cap")` uses 10-character string, exceeding Soroban's 9-char `symbol_short!` limit. Changed to `symbol_short!("mbt_cap")` (7 chars).  
**File**: `contracts/predictify-hybrid/src/events.rs` line 1145  
**Resolution**: Renamed symbol to `"mbt_cap"` using `symbol_short!` macro for compile-time validation.  

### Issue: Missing nonce fields in event structs
**Status**: ✅ RESOLVED  
**Description**: `ResolutionTimeoutEvent` and `DisputeTimeoutExpiredEvent` structs were missing `nonce` and `timestamp` fields required by event schema.  
**Files**: `contracts/predictify-hybrid/src/events.rs` lines 548-565  
**Resolution**: Added `nonce: u64` and `timestamp: u64` fields to both structs; updated emit functions to populate them.  

### Issue: RecoveryTimelockManager type not found
**Status**: ✅ RESOLVED  
**Description**: 5 call sites in lib.rs referenced non-existent `crate::recovery::RecoveryTimelockManager` type. Actual type is `RecoveryTimelockConfig`.  
**Files**: `contracts/predictify-hybrid/src/lib.rs` lines 5103, 5138, 5167, 5191, 5198  
**Resolution**: Replaced all 5 references of `RecoveryTimelockManager` with `RecoveryTimelockConfig`.  

### Issue: Duplicate vec imports in recovery.rs tests
**Status**: ✅ RESOLVED  
**Description**: `use soroban_sdk::vec;` appeared twice in `contracts/predictify-hybrid/src/recovery.rs` tests module.  
**File**: `contracts/predictify-hybrid/src/recovery.rs` lines ~938-940  
**Resolution**: Removed duplicate import; kept single `use soroban_sdk::vec;`.  

### Issue: Duplicate mod declarations in lib.rs
**Status**: ✅ RESOLVED  
**Description**: Modules declared multiple times causing compile error E0428: `mod events;`, `mod extensions;`, `mod tokens;`, `mod audit_trail;`, `mod monitor;`.  
**File**: `contracts/predictify-hybrid/src/lib.rs` lines 30-94  
**Resolution**: Removed duplicate declarations; kept single definition for each module.  

### Issue: Duplicate emit_ functions in events.rs
**Status**: ✅ RESOLVED  
**Description**: Multiple emit functions appeared twice in the EventEmitter impl block (compact versions followed by expanded versions with schema registry).  
**File**: `contracts/predictify-hybrid/src/events.rs` (first impl block ~lines 1040-2250, second impl block ~lines 3270+)  
**Functions removed**:
  - `emit_market_closed` (1272, kept 1422)
  - `emit_refund_on_oracle_failure` (1282, kept 1441)
  - `emit_state_change_event` (1292, kept 1566)
  - `emit_winnings_claimed` (1302, kept 1613)
  - `emit_oracle_admin_cooldown_hit` (1459, kept 1370)
  - `emit_monitor_queue_overflow` (1486, kept 2217)
  - `emit_balance_changed` (1490, kept 2147)
  - `emit_oracle_median_quotes` (1196, kept 3369)
  - `emit_threshold_proposed` (3275, kept 1389)
  - `emit_threshold_confirmed` (3299, kept 1399)
**Resolution**: Removed all duplicate function definitions, keeping the most complete/expanded version.  

### Issue: Missing emit functions for dispute events
**Status**: ✅ RESOLVED  
**Description**: Three emit functions referenced in `contracts/predictify-hybrid/src/disputes.rs` were not implemented in events.rs:
  - `emit_dispute_timeout_set`
  - `emit_dispute_timeout_expired`
  - `emit_dispute_timeout_extended`
  - `emit_suspected_collusion_flag`
  - `emit_dispute_vote_rejected`
  - `emit_dispute_auto_resolved`
**File**: `contracts/predictify-hybrid/src/events.rs`  
**Resolution**: Added all 6 missing emit functions with proper nonce/timestamp handling.  

---

## Future Issues (To Be Filed)

When adding new features or fixing bugs, consider filing these as individual issues:

- **Error code stability checks for non-hybrid contracts**: Allowlist, Reporting contracts lack `err_stability.rs` (see Issue #103)
- **Rust toolchain pinning for fuzz targets**: No `rust-toolchain.toml` pinning nightly version (see Issue #104)
- **Module organization**: Consider consolidating duplicate module declarations patterns across contracts
- **Event schema registry consistency**: Document and enforce pattern for new events
