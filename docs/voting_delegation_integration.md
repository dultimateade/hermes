# Voting and Delegation Integration Specification

## Overview

The voting system integrates with the delegation registry to enable voting on behalf of delegated accounts while preserving vote attribution and security.

## Architecture

### Authority Resolution

When a vote is cast, the system resolves the actual voting authority based on delegation status:

```
User attempts to vote
    ↓
Check delegation status
    ↓
├─ No delegation → Vote as user (direct voting)
├─ User is delegator → Can vote directly (optional)
└─ User is delegate → Vote attributed to delegator
```

### Vote Attribution

All votes are attributed to the account with voting rights (the delegator), not the account executing the vote (the delegate):

```
Physical Vote Execution:
  - Delegate casts vote via on-chain transaction
  - Requires delegate.require_auth()

Vote Attribution:
  - Vote stake attributed to delegator
  - Delegator receives winnings
  - Voting analytics credit delegator
```

## Implementation Points

### 1. Vote Processing

When a vote is cast via `vote_on_dispute()` or equivalent voting entrypoint:

```rust
// Before: Direct voting only
fn vote_on_dispute(env: Env, user: Address, market_id: Symbol, vote: String, stake: i128) {
    user.require_auth();  // Only user can vote
    // Process vote for user
}

// After: Delegation-aware voting
fn vote_on_dispute(env: Env, user: Address, market_id: Symbol, vote: String, stake: i128) {
    // Resolve voting authority
    let authority = voting_delegation_integration::VotingIntegration::resolve_voting_authority(
        &env, &user
    )?;
    
    user.require_auth();  // Current voter (user or delegate) authorizes
    
    // Vote is attributed to authority.vote_owner
    // Not authority.executing_authority
}
```

### 2. Delegation Checks in Voting

Integration points for delegation checks:

1. **Vote Authority Resolution** (required)
   ```rust
   let authority = VotingIntegration::resolve_voting_authority(&env, voter)?;
   voter.require_auth();  // Actual voter authorizes
   // Use authority.vote_owner for attribution
   ```

2. **Vote Eligibility Check** (optional)
   ```rust
   VotingIntegration::can_vote(&env, voter)?;  // Check if eligible to vote
   ```

3. **Delegated Vote Detection** (optional)
   ```rust
   if VotingIntegration::is_delegated_vote(&env, voter) {
       // Track delegated vs. direct votes
   }
   ```

### 3. Event Emission

When a delegated vote is cast, emit appropriate events:

```rust
// Event: vote_cast
// Attributes:
//   - voter: address that executed the vote (delegate)
//   - vote_owner: address to attribute vote to (delegator)
//   - market_id: market being voted on
//   - stake: voting stake
//   - is_delegated: whether this is a delegated vote

env.events().publish(
    ("vote_cast",),
    (voter, vote_owner, market_id, stake, is_delegated)
);
```

### 4. Winnings Attribution

Ensure winnings are properly attributed:

```rust
// Winnings calculation uses vote_owner (delegator)
fn calculate_user_payout(env: &Env, market: &Market, voter: &Address) -> Result<i128, Error> {
    // Get authority info
    let authority = VotingIntegration::resolve_voting_authority(env, voter)?;
    
    // Calculate payout for vote_owner (delegator)
    let user_stake = market.stakes.get(authority.vote_owner.clone()).unwrap_or(0);
    // ... rest of calculation
}

// Winnings transfer to vote_owner (delegator)
VotingUtils::transfer_winnings(env, &authority.vote_owner, payout)?;
```

### 5. Query Functions

Voting analytics should account for delegation:

```rust
// Query voting by account
// Returns votes cast directly + votes cast on behalf (if applicable)
fn get_user_votes(env: &Env, user: &Address) -> Result<Vec<Vote>, Error> {
    // Votes where user is vote_owner (delegator)
    // Note: Does not include votes where user is executing_authority (delegate)
}

// Query votes for delegator
// Returns votes cast by user OR by their delegate(s)
fn get_delegator_votes(env: &Env, delegator: &Address) -> Result<Vec<Vote>, Error> {
    // Votes where delegator is vote_owner
    // Includes votes cast by delegator directly
    // Includes votes cast by delegate on behalf of delegator
}
```

## Voting Scenarios

### Scenario 1: Direct Voting (No Delegation)

```
User A votes directly on market M
    ↓
voter = A
vote_owner = A
executing_authority = A
is_delegated = false
    ↓
A.require_auth() succeeds
Vote attributed to A
```

### Scenario 2: Delegated Voting (Delegate Votes)

```
User A delegates to User B
User B votes on behalf of A on market M
    ↓
voter = B (executing)
vote_owner = A (delegator, gets credit)
executing_authority = B (delegate)
is_delegated = true
    ↓
B.require_auth() succeeds
Vote attributed to A
Winnings credited to A
```

### Scenario 3: Delegator Still Votes

```
User A delegates to User B
User A votes directly on same market M
    ↓
First vote:
  voter = B, vote_owner = A, is_delegated = true

Second vote:
  voter = A, vote_owner = A, is_delegated = false
    ↓
Both votes attributed to A
Combined winnings to A
```

## Error Handling

### Voting with Invalid Delegation

```rust
if user is marked as voted in a market AND
   is_delegated vote {
    // Check if same delegator already voted directly
    if direct_vote_exists {
        // This is fine - delegator and delegate can both vote
        return Ok(());
    }
}
```

### Circular Delegation Issues

Circular delegation prevention in the delegation system prevents:
- A→B then B→A: Prevented at delegation time
- Longer cycles: Not prevented (documented limitation)

For voting, no additional checks needed - rely on delegation system constraints.

## Gas Optimization

### Delegation Check Cost

```rust
// Fast: O(1) lookup
let active_delegate = DelegationManager::get_active_delegate(&env, &user);

// Expensive: O(n) iteration (not recommended in voting path)
let all_delegations = DelegationManager::get_delegations_by_delegator(&env, &user);
```

### Caching Strategy

```rust
// Client-side: Cache delegation status
let delegation = cache.get_or_compute(&user, || {
    DelegationManager::get_active_delegate(&env, &user)
});
```

## Testing Strategy

### Unit Tests

1. **Direct Voting**: Verify vote_owner == user when no delegation
2. **Delegated Voting**: Verify vote_owner == delegator when user is delegate
3. **Double Voting**: Verify delegator can still vote directly after delegation
4. **Authority Resolution**: Test all delegation scenarios

### Integration Tests

1. **Delegation + Market Voting**: Create delegation, vote, claim winnings
2. **Revocation During Voting**: Revoke delegation, verify voting unaffected
3. **Multi-Market**: Delegator votes via delegate on multiple markets
4. **Event Emission**: Verify correct events emitted for delegated votes

## Deployment Checklist

- [ ] Add `voting_delegation_integration` module to contract
- [ ] Integrate delegation checks into voting entrypoints
- [ ] Update vote attribution to use `vote_owner` from delegation check
- [ ] Update winnings calculation to use delegator address
- [ ] Emit events with delegation information
- [ ] Update voting analytics queries
- [ ] Add integration tests
- [ ] Update documentation and API specs
- [ ] Test on testnet with delegation + voting scenarios
- [ ] Deploy to mainnet with voting delegation support

## Backward Compatibility

Voting delegation integration is backward compatible:
- Users without delegations vote as before (direct voting)
- Existing votes and winnings unaffected
- Delegation is opt-in feature
- Non-delegating users see no behavior change

## Future Enhancements

1. **Delegation Expiry**: Add time-based expiration for delegations
2. **Cooldown Periods**: Add revocation cooldowns for security
3. **Delegation Pools**: Allow one delegate to represent multiple delegators
4. **Voting Restrictions**: Limit what delegates can vote on
5. **Vote Signing**: Require cold storage co-signature for delegated votes
