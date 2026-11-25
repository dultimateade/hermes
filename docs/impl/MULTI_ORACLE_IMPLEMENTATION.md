# Multi-Oracle Resolution Implementation

## Overview

This document describes the multi-oracle resolution system implemented in the Hermes markets contract. The system enables high-stakes prediction markets to use multiple independent oracles with median aggregation to improve resilience against oracle failures and manipulation.

## Problem Statement

Single-oracle resolution mechanisms are vulnerable to:
- **Oracle failures**: A single oracle going offline prevents market resolution
- **Manipulation**: One oracle can unilaterally determine the outcome, creating incentives for manipulation
- **Systemic risk**: Market participants are entirely dependent on the reliability of a single oracle

The multi-oracle resolution system addresses these issues by:
- Accepting outcomes from 3 or more independent oracle sources
- Using median aggregation to resist individual oracle manipulation
- Requiring consensus-driven resolution

## Architecture

### Data Structures

#### OracleSubmission
Represents a single oracle's outcome submission for a market:
```rust
pub struct OracleSubmission {
    pub oracle: Address,           // Address of the oracle
    pub outcome: u32,               // Submitted outcome index
    pub submitted_at: u64,          // Ledger sequence timestamp
}
```

#### MarketData (Updated)
Extended with multi-oracle support:
```rust
pub struct MarketData {
    // ... existing fields ...
    pub required_oracles: u32,  // 0 = single oracle, 3+ = multi-oracle
}
```

#### DataKey (Updated)
New storage keys for oracle tracking:
```rust
pub enum DataKey {
    // ... existing keys ...
    OracleSubmission(u32, Address),  // (market_id, oracle_address)
    MarketOracles(u32),              // (market_id)
}
```

### Error Codes

New error types specific to multi-oracle resolution:
- **15**: `InsufficientOracleSubmissions` - Not enough oracles submitted
- **16**: `OracleAlreadySubmitted` - Oracle submitted twice for same market
- **17**: `InvalidOracleOutcome` - Oracle submitted outcome outside valid range
- **18**: `DuplicateOracle` - Same oracle address added multiple times
- **19**: `InsufficientOraclesConfigured` - Market requires at least 3 oracles

## Public API

### 1. create_market (Updated)

Enhanced to support multi-oracle configuration:

```rust
pub fn create_market(
    env: Env,
    creator: Address,
    question: String,
    description: String,
    end_time: u64,
    resolution_source: String,
    outcome_tags: Vec<String>,
    required_oracles: u32,  // NEW: 0 = single oracle, 3+ = multi-oracle
) -> u32
```

**Validation**:
- `required_oracles` must be 0 or >= 3
- Values of 1 or 2 are rejected with `InvalidConfig`

**Auth**: Requires market creator authentication

### 2. submit_oracle_outcome

New entrypoint for oracle outcome submission:

```rust
pub fn submit_oracle_outcome(
    env: Env,
    oracle: Address,
    market_id: u32,
    outcome: u32,
) -> ()
```

**Parameters**:
- `oracle`: The oracle's address (must sign the transaction)
- `market_id`: Target market ID
- `outcome`: The predicted outcome index (0-based)

**Validation**:
- Market must exist
- Market must be configured for multi-oracle (`required_oracles >= 3`)
- Market must not already be resolved
- Outcome must be valid (< number of outcomes)
- Oracle must not have already submitted for this market

**Auth**: Requires oracle authentication

**Returns**: None (state update)

**Error Cases**:
- `MarketNotFound` - Market doesn't exist
- `InvalidConfig` - Market is not multi-oracle configured
- `MarketAlreadyResolved` - Market already resolved
- `InvalidOracleOutcome` - Outcome out of range
- `OracleAlreadySubmitted` - Oracle already submitted for this market
- `Unauthorized` - Oracle didn't authenticate

### 3. resolve_market_with_oracles

New entrypoint for multi-oracle resolution:

```rust
pub fn resolve_market_with_oracles(
    env: Env,
    resolver: Address,
    market_id: u32,
    oracle_addresses: Vec<Address>,
) -> ()
```

**Parameters**:
- `resolver`: Authorized resolver (typically market creator)
- `market_id`: Target market ID
- `oracle_addresses`: Vector of oracle addresses that submitted outcomes

**Algorithm**:
1. Verify market exists and is multi-oracle configured
2. Collect outcomes from all oracle submissions
3. Sort outcomes using bubble sort (efficient for small n)
4. Compute median: for odd-length arrays, return middle; for even-length, return lower-middle
5. Record winning outcome and mark market resolved

**Validation**:
- Market must exist
- Market must be configured for multi-oracle
- Market must not already be resolved
- At least `required_oracles` submissions must be available

**Auth**: Requires resolver (market creator) authentication

**Returns**: None (state update)

**Error Cases**:
- `MarketNotFound` - Market doesn't exist
- `InvalidConfig` - Market is not multi-oracle configured
- `MarketAlreadyResolved` - Market already resolved
- `InsufficientOracleSubmissions` - Too few oracle submissions
- `Unauthorized` - Resolver didn't authenticate

## Algorithm Details

### Median Aggregation

The system uses median aggregation to determine the consensus outcome:

**Example 1: Odd number of submissions**
- Submissions: [0, 1, 2]
- Sorted: [0, 1, 2]
- Median index: (3-1)/2 = 1
- Median: 1 ✓

**Example 2: Even number of submissions**
- Submissions: [0, 1, 1, 2]
- Sorted: [0, 1, 1, 2]
- Median index: (4-1)/2 = 1
- Median: 1 ✓

**Example 3: Manipulation resistance**
- Submissions: [0, 0, 0, 2] (3 honest, 1 malicious)
- Sorted: [0, 0, 0, 2]
- Median index: (4-1)/2 = 1
- Median: 0 ✓ (honest consensus wins)

### Sorting Algorithm

Implementation uses bubble sort optimized for small vectors (typical oracle count: 3-7):

```rust
// Create a mutable copy for sorting
let mut sorted: Vec<u32> = outcomes.clone();

// Bubble sort for small vectors
for i in 0..len {
    for j in 0..(len - i - 1) {
        if sorted.get_unchecked(j) > sorted.get_unchecked(j + 1) {
            let temp = sorted.get_unchecked(j);
            sorted.set(j, sorted.get_unchecked(j + 1));
            sorted.set(j + 1, temp);
        }
    }
}

// Return median
let median_index = (len - 1) / 2;
sorted.get_unchecked(median_index)
```

**Time Complexity**: O(n²) for n oracle submissions (acceptable for n ≤ 10)

## Test Coverage

### Unit Tests (multi_oracle.rs)

18 comprehensive test cases:

1. **Basic Functionality**
   - `test_submit_oracle_outcome_success()` - Valid submission succeeds
   - `test_resolve_market_with_oracles_median_odd()` - Median with 3 oracles
   - `test_resolve_market_with_oracles_median_even()` - Median with 4+ oracles

2. **Validation & Constraints**
   - `test_submit_oracle_outcome_duplicate_fails()` - Duplicate submissions rejected
   - `test_submit_oracle_outcome_invalid_outcome()` - Out-of-range outcomes rejected
   - `test_submit_oracle_outcome_market_not_found()` - Non-existent market rejected
   - `test_create_market_invalid_oracle_count()` - Only 0 or 3+ allowed

3. **Market Type Enforcement**
   - `test_submit_oracle_outcome_single_oracle_market_fails()` - Can't submit to single-oracle market
   - `test_resolve_market_with_oracles_single_oracle_market()` - Can't resolve single-oracle market

4. **State Management**
   - `test_resolve_market_with_oracles_already_resolved()` - Can't resolve twice
   - `test_submit_oracle_outcome_after_resolved()` - Can't submit after resolution
   - `test_resolve_market_with_oracles_insufficient()` - Rejects insufficient submissions

5. **Security & Resilience**
   - `test_resolve_market_with_oracles_manipulation_resistance()` - Median resists manipulation
   - `test_resolve_market_with_oracles_five()` - Scalability with 5 oracles

### Auth Boundary Tests (auth_boundary.rs)

4 new tests ensuring proper authentication enforcement:

1. `test_submit_oracle_outcome_requires_auth()` - Unauthorized oracle rejected
2. `test_submit_oracle_outcome_requires_auth_success()` - Authorized oracle succeeds
3. `test_resolve_market_with_oracles_requires_auth()` - Unauthorized resolver rejected
4. `test_resolve_market_with_oracles_requires_auth_success()` - Authorized resolver succeeds

## Usage Workflow

### Creating a Multi-Oracle Market

```
1. Market creator calls create_market(..., required_oracles=3)
   - Returns market_id
2. Market exists in state, ready for oracle submissions
```

### Multi-Oracle Resolution

```
1. Oracle 1 calls submit_oracle_outcome(market_id=1, outcome=0)
   - Oracle submission recorded
2. Oracle 2 calls submit_oracle_outcome(market_id=1, outcome=1)
   - Oracle submission recorded
3. Oracle 3 calls submit_oracle_outcome(market_id=1, outcome=0)
   - Oracle submission recorded
4. Market creator calls resolve_market_with_oracles(
     market_id=1, 
     oracle_addresses=[oracle1, oracle2, oracle3]
   )
   - Outcomes [0, 1, 0] sorted to [0, 0, 1]
   - Median = 0
   - Market resolved with outcome 0
5. Bettors on outcome 0 can claim winnings
```

## Security Considerations

### Manipulation Resistance

- **Byzantine tolerance**: With 3 honest oracles and 1 malicious, honest consensus wins
- **Quorum requirement**: Must have at least required_oracles submissions to resolve
- **Deterministic aggregation**: Median is always computable and non-manipulable

### Authentication

- Each oracle's submission requires that oracle's signature (require_auth)
- Resolution requires market creator's signature
- Prevents unauthorized outcome submission

### State Integrity

- Markets are marked `resolved=true` after resolution
- Cannot resolve twice (enforced with state check)
- Cannot submit after resolution (enforced with state check)
- Each oracle can submit at most once per market (enforced with duplicate check)

## Future Enhancements

Potential improvements for future versions:

1. **Weighted Oracles**: Support oracle reputation scores
2. **Sliding Window**: Allow new oracle submissions during a time window
3. **Partial Quorum**: Allow resolution with > required_oracles (e.g., 4 of 5)
4. **Dynamic Oracle Set**: Add/remove oracles without creating new markets
5. **Appeal Mechanism**: Allow market participants to challenge resolution
6. **Gas Optimization**: Use more efficient sorting for larger oracle sets

## Integration with Existing Systems

The multi-oracle system integrates seamlessly with existing market functionality:

- **Betting**: Users can place bets on multi-oracle markets normally
- **Liquidity**: Liquidity provision works identically
- **Winnings**: Claim winnings uses same logic post-resolution
- **Cancellation**: Markets can be cancelled before resolution (both types)

## Error Handling Guide

| Error | Cause | Resolution |
|-------|-------|-----------|
| InvalidConfig | Market created with 1-2 oracles | Create market with 0 or 3+ oracles |
| OracleAlreadySubmitted | Oracle submitted twice | Use different oracle address or wait for next market |
| InvalidOracleOutcome | Outcome outside valid range | Submit outcome index < number of outcomes |
| InsufficientOracleSubmissions | Too few submissions to resolve | Wait for more oracle submissions |
| MarketAlreadyResolved | Attempted to resolve twice | Cannot resolve again after resolution |
| Unauthorized | Missing required signature | Ensure proper authentication for the caller |

## Conclusion

The multi-oracle resolution system provides a robust, decentralized mechanism for high-stakes market resolution. By requiring consensus from multiple independent sources and using proven median aggregation, the system improves resilience, reduces manipulation incentives, and maintains backward compatibility with single-oracle markets.
