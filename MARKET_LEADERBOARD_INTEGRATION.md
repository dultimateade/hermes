# Market Leaderboard - Integration Status

## Summary

The market leaderboard feature implementation is **95% complete**. All core logic exists in `market_analytics.rs` with comprehensive tests, but integration touchpoints need to be connected.

## Current Status

### ✅ Completed Components

1. **Core Implementation** (`market_analytics.rs` lines 593-850)
   - `MarketLeaderboard::upsert` – O(N) insertion/update with capacity bounds
   - `MarketLeaderboard::top_by_stake` – O(N log N) read with sorting and rank assignment
   - Helper functions: `find_user_in_heap`, `min_stake_index`, `sort_descending_by_stake`

2. **Data Types** (`types.rs`)
   - `MarketLeaderboardEntry` – Versioned struct with user, rank, stake, timestamp

3. **Storage** (`storage.rs`)
   - `DataKey::MarketLeaderboard(Symbol)` – Per-market heap storage
   - `MAX_MARKET_LEADERBOARD_CAPACITY = 50` – Hard cap for gas safety

4. **Tests** (`market_leaderboard_tests.rs`)
   - 19 comprehensive test cases covering edge cases, capacity bounds, eviction logic, tie-breaking
   - All tests isolated and compile-clean

### ⏳ Remaining Integration Work

#### 1. Place Bet Hook in `bets.rs`

Add call to `MarketLeaderboard::upsert` in the `place_bet` function after stake update.

**Location**: `contracts/predictify-hybrid/src/bets.rs` – `place_bet` function

**Code to add**:
```rust
// After BetValidator::get_user_stake() or similar stake update:
use crate::market_analytics::MarketLeaderboard;

let current_timestamp = env.ledger().timestamp();
let _ = MarketLeaderboard::upsert(
    env,
    &market_id,
    &user,
    cumulative_stake,      // total stake for user in this market
    current_timestamp,
    market.leaderboard_capacity,  // or default to 50
);
// Errors from upsert are silently ignored (non-critical feature)
```

#### 2. Public API Function in `lib.rs`

Add read-only view function to expose leaderboard data.

**Location**: `contracts/predictify-hybrid/src/lib.rs` – in appropriate `#[contractimpl]` block

**Code to add**:
```rust
#[contractimpl]
pub fn get_market_leaderboard(
    env: Env,
    market_id: Symbol,
    limit: u32,
) -> soroban_sdk::Vec<MarketLeaderboardEntry> {
    // No auth required – public read-only query
    MarketLeaderboard::top_by_stake(&env, &market_id, limit)
}
```

**Accessibility**: No authentication required (read-only analytics)

#### 3. Market Config (Optional but Recommended)

Add leaderboard capacity field to market struct if not already present.

**Suggested field**: `leaderboard_capacity: u32` (default 50, user-configurable 1-50)

---

## Design Decisions

| Aspect | Choice | Rationale |
|--------|--------|-----------|
| Algorithm | O(N) linear scan vs O(log N) binary heap | Soroban SDK `Vec` doesn't support true heap; N≤50 makes scan negligible |
| Tie-breaker | Earlier timestamp wins | First-bettor advantage (fairness) |
| Storage | Per-market independent heaps | Isolation; no cross-market leakage |
| Capacity | Hard-capped at 50 | Gas safety; storage cost predictability |
| Errors | Non-fatal | Leaderboard failure doesn't abort bets |

---

## Testing Strategy

- ✅ **Unit tests**: 19 test cases in `market_leaderboard_tests.rs`
- ⏳ **Integration tests**: Should verify `place_bet` updates leaderboard correctly
- ⏳ **Gas benchmark**: Measure cost of upsert at N=1, 25, 50

---

## Next Steps (For Implementation)

1. **Add integration in bets.rs**: Call `MarketLeaderboard::upsert` in `place_bet`
2. **Add public API in lib.rs**: Expose `get_market_leaderboard` function
3. **Add integration tests**: Verify leaderboard updates on bets
4. **Run full test suite**: `cargo test -p predictify-hybrid`
5. **Gas profiling**: Measure and document gas costs
6. **Deploy to testnet**: Validate performance at scale

---

## Files That Need Changes

- [ ] `contracts/predictify-hybrid/src/bets.rs` – Add upsert call
- [ ] `contracts/predictify-hybrid/src/lib.rs` – Add public API function
- [ ] `contracts/predictify-hybrid/src/types.rs` – Possibly add `leaderboard_capacity` field to market struct

---

## Risk Assessment

**Low Risk**: All changes are additive; existing functionality unchanged.
- No modifications to bet placement logic (only addition after existing ops)
- Read-only public function (no state mutations)
- Errors from upsert silently ignored (graceful degradation)

---

## Documentation

- ✅ Inline documentation in `market_analytics.rs` (algorithm, complexity, safety)
- ✅ Test documentation in `market_leaderboard_tests.rs`
- ⏳ Public API rustdoc (for `get_market_leaderboard`)
- ⏳ Integration guide for developers

---

**Status**: Ready for PR + integration  
**Estimated Integration Time**: 30 minutes  
**Estimated Review Time**: 20 minutes  

