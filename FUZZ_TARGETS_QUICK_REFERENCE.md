# Fuzz Targets Quick Reference

## Files Created (798 lines total)

| File | Lines | Purpose |
|------|-------|---------|
| `contracts/markets/fuzz/Cargo.toml` | 22 | Fuzz infrastructure for markets |
| `contracts/markets/fuzz/targets/place_bet.rs` | 219 | Fuzz target for `place_bet` entrypoint |
| `contracts/markets/fuzz/targets/claim_winnings.rs` | 259 | Fuzz target for `claim_winnings` entrypoint |
| `contracts/fees/fuzz/Cargo.toml` | 18 | Fuzz infrastructure for fees |
| `contracts/fees/fuzz/targets/collect_fees.rs` | 280 | Fuzz target for `collect_fees` entrypoint |

## Quick Start

### Run individual fuzz targets:
```bash
# Markets contract - place_bet (bet placement logic)
cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet

# Markets contract - claim_winnings (payout claim logic with CEI protection)
cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz claim_winnings

# Fees contract - collect_fees (fee collection logic)
cargo +nightly fuzz run --fuzz-dir contracts/fees/fuzz collect_fees
```

### Run with time limit:
```bash
timeout 3600 cargo +nightly fuzz run --fuzz-dir contracts/markets/fuzz place_bet
```

## Risk Surface Coverage

### place_bet.rs
- **Tests**: Bet placement with arbitrary stakes, market states, outcome validation
- **Actions**: place_bet, resolve_market, cancel_market
- **Errors frozen**: StakeTooSmall, InvalidOutcome, InvalidState, MarketNotFound, Overflow

### claim_winnings.rs
- **Tests**: Payout claims with CEI pattern protection, double-claim detection
- **Actions**: place_bet, resolve_market, claim_winnings, advance_time
- **Errors frozen**: InvalidState, MarketNotFound, MarketNotResolved, InvalidOutcome, AlreadyClaimed

### collect_fees.rs
- **Tests**: Fee collection with admin enforcement, cooldown periods, collection thresholds
- **Actions**: initialize, record_fee, collect_fees, advance_time, pause_fees, unpause_fees
- **Errors frozen**: Unauthorized, AdminNotSet, FeesPaused, BelowCollectionThreshold, WithdrawalCooldownActive, InvalidState

## Architecture

Each fuzz target:
1. Uses libfuzzer-sys for input generation
2. Follows fixed-byte-consumption pattern for deterministic action sequencing
3. Pre-allocates limited resource pools (4 markets, 5 users, etc.)
4. Validates all errors against frozen error codes
5. Panics on unexpected errors to catch regression
6. Implements CEI (checks-effects-interactions) pattern validation where applicable

## Security Highlights

- **place_bet**: Detects overflow on pool accumulation, validates market state transitions
- **claim_winnings**: Enforces double-claim prevention (CEI pattern), validates payout calculations
- **collect_fees**: Enforces admin-only operations, cooldown enforcement, threshold validation

## Related Documentation

See `/workspaces/hermes/FUZZ_TARGETS_IMPLEMENTATION.md` for detailed implementation guide.
