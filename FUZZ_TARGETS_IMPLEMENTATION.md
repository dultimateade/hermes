# Fuzz Targets Implementation Summary

## Overview

Added libfuzzer-based fuzz targets for the markets and fees contracts to improve security coverage of high-risk surfaces. The markets contract handles the most funds and has the highest attack surface, making comprehensive fuzzing critical.

## Files Created

### Markets Contract Fuzz Targets

#### `/workspaces/hermes/contracts/markets/fuzz/Cargo.toml`
- Configured libfuzzer-sys and markets dependency
- Set up two binary targets: `place_bet` and `claim_winnings`

#### `/workspaces/hermes/contracts/markets/fuzz/targets/place_bet.rs`
- **Coverage**: Fuzzes the `place_bet` entrypoint with arbitrary input sequences
- **Scenarios Tested**:
  - Arbitrary stake amounts (zero, negative, small, large, i128::MAX)
  - Market states (active, resolved, cancelled)
  - Outcome index validation (valid/invalid ranges)
  - Multiple user sequences (same user, different users)
  - Pool accumulation and overflow detection
- **Actions** (3 types):
  1. `place_bet` - Place a bet with fuzzed user, market, outcome, stake
  2. `resolve_market` - Transition market to resolved state
  3. `cancel_market` - Transition market to cancelled state
- **Expected Errors**: StakeTooSmall, InvalidOutcome, InvalidState, MarketNotFound, Overflow
- **Boundary Conditions**:
  - Stake <= 0 rejected
  - outcome_idx >= num_outcomes rejected
  - Resolved/cancelled markets reject new bets
  - Pool accumulation with overflow detection

#### `/workspaces/hermes/contracts/markets/fuzz/targets/claim_winnings.rs`
- **Coverage**: Fuzzes the `claim_winnings` entrypoint with focus on CEI pattern
- **Scenarios Tested**:
  - Market states (unresolved, resolved, cancelled)
  - User roles (bettor with winning/losing bets, non-bettor)
  - Double-claim detection (critical CEI pattern)
  - Payout calculations (gross, LP fee, remainder)
- **Actions** (4 types):
  1. `place_bet` - Establish user positions
  2. `resolve_market` - Mark market resolved with winning outcome
  3. `claim_winnings` - Claim rewards (fuzzes user and market)
  4. `advance_time` - Progress ledger timestamp
- **Expected Errors**: InvalidState, MarketNotFound, MarketNotResolved, InvalidOutcome, AlreadyClaimed
- **Boundary Conditions**:
  - Cannot claim before market resolved
  - Cannot claim twice (CEI pattern prevents double-claim)
  - Losing bets yield InvalidOutcome
  - Payout remainder tracking accumulates fractional stroops

### Fees Contract Fuzz Targets

#### `/workspaces/hermes/contracts/fees/fuzz/Cargo.toml`
- Configured libfuzzer-sys and fees dependency
- Set up single binary target: `collect_fees`

#### `/workspaces/hermes/contracts/fees/fuzz/targets/collect_fees.rs`
- **Coverage**: Fuzzes the `collect_fees` entrypoint with various fee configurations
- **Scenarios Tested**:
  - Auth & admin enforcement
  - Collection threshold boundaries (below, at, above threshold)
  - Fee configuration states (enabled/paused)
  - Withdrawal schedule cooldown enforcement
  - Fee accumulation and reset behavior
  - Edge cases (zero collected, overflow scenarios)
- **Actions** (6 types):
  1. `initialize` - Initialize contract (idempotent)
  2. `record_fee` - Accumulate fees from authorized callers
  3. `collect_fees` - Collect accumulated fees (requires admin)
  4. `advance_time` - Progress ledger timestamp
  5. `pause_fees` - Pause fee operations
  6. `unpause_fees` - Resume fee operations
- **Expected Errors**: Unauthorized, AdminNotSet, FeesPaused, BelowCollectionThreshold, WithdrawalCooldownActive, InvalidState
- **Boundary Conditions**:
  - Unauthorized non-admin callers rejected
  - Collection below threshold rejected
  - Cooldown period enforced
  - Fees paused blocks collection
  - Overflow detection on accumulated fees

## Implementation Details

### Architecture Consistency
All fuzz targets follow the established pattern from existing targets in:
- `contracts/disputes/fuzz/targets/main.rs`
- `contracts/oracles/fuzz/targets/main.rs`
- `contracts/resolution/fuzz/targets/main.rs`
- `contracts/predictify-hybrid/fuzz/targets/resolve.rs`

### Key Design Decisions

1. **Fixed-size byte consumption**: Each action type consumes a fixed number of bytes from the input corpus, allowing the fuzzer to learn structure without an explicit grammar

2. **Deterministic state pools**: Pre-allocate 4-5 markets/users/admins to explore combinations without requiring unbounded resource allocation

3. **CEI Pattern Protection**: The `claim_winnings` target specifically tests the checks-effects-interactions pattern with pre-claim flag validation

4. **Error Freezing**: All error codes match frozen values in existing auth_boundary tests (error_stab.rs), preventing silent regression

5. **Panic Detection**: Any unexpected error causes a crash, ensuring all execution paths remain within expected error set

## Running the Fuzz Targets

### Prerequisites
```bash
# Install nightly Rust and cargo-fuzz
rustup toolchain install nightly
cargo install cargo-fuzz
```

### Running Individual Targets
```bash
# Markets contract - place_bet
cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet

# Markets contract - claim_winnings
cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz claim_winnings

# Fees contract - collect_fees
cargo +nightly fuzz run --fuzz-dir contracts/fees/fuzz collect_fees
```

### Running with Corpus Growth
```bash
# Run for 1 hour, accumulating corpus
timeout 3600 cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet

# Run against accumulated corpus
cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet
```

## Risk Coverage Improvement

### Before
- ✓ disputes
- ✓ oracles
- ✓ resolution
- ✓ predictify-hybrid
- ✗ **markets** (handles most funds - CRITICAL)
- ✗ **fees** (collects platform revenue - HIGH)
- ✗ admin
- ✗ allowlist
- ✗ validators

### After
- ✓ disputes
- ✓ oracles
- ✓ resolution
- ✓ predictify-hybrid
- ✓ **markets** (NEW - place_bet, claim_winnings)
- ✓ **fees** (NEW - collect_fees)
- ✗ admin
- ✗ allowlist
- ✗ validators

## Verification Status

All fuzz targets have been:
- ✓ Syntactically verified
- ✓ Import paths verified against existing tests
- ✓ Contract method signatures verified
- ✓ Error codes frozen against auth_boundary tests
- ✓ CEI pattern protection validated

Ready for CI validation via GitHub Actions with nightly Rust toolchain.
