# Voting Delegation Registry - Security and Implementation Guide

## Overview

The voting delegation registry enables institutional participants to maintain voting rights in cold storage while delegating active voting authority to dedicated hot wallets. This architecture significantly improves security by reducing the exposure of high-value accounts to operational risks.

## Architecture

### Security Model

The delegation system is built on four core principles:

1. **Delegator-Controlled Authority**: Only the delegator (cold storage account) can establish or revoke delegations
2. **Single Active Delegate**: Each delegator can have only one active delegate at any given time
3. **Non-Circular**: The system prevents delegation chains that would create voting authority loops
4. **Non-Self**: An account cannot delegate to itself

### Data Model

The delegation registry maintains two types of records:

```
Delegations: (delegator, delegate) -> Delegation {
    delegator: Address,
    delegate: Address,
    activated_at: u64,           // Ledger timestamp
    last_modified: u64,          // Last update timestamp
}

Active Delegate Index: delegator -> delegate
RevocationHistory: (delegator, delegate) -> Vec<Revocation>
```

### Entrypoints

#### State-Changing Operations (Require Authorization)

**`delegate_votes(delegator: Address, delegate: Address) -> Result<(), Error>`**

Establishes a voting delegation from the delegator to the delegate. If the delegator already has an active delegation to another address, it is automatically replaced.

- **Authorization**: Requires `delegator.require_auth()`
- **Events**: Emits `set_delegate` governance event
- **Constraints**:
  - Cannot delegate to self
  - Cannot create circular delegation (if B delegates to A, A cannot delegate to B)
- **Use Case**: Cold storage account delegates voting to hot wallet

```rust
// Example: Cold storage delegates to hot wallet
PredictifyHybrid::delegate_votes(
    env,
    cold_storage_address,    // delegator
    hot_wallet_address,      // delegate
)?;
```

**`unset_delegate(delegator: Address, delegate: Address) -> Result<(), Error>`**

Revokes a delegation by the delegator. This is the primary revocation mechanism for delegators.

- **Authorization**: Requires `delegator.require_auth()`
- **Events**: Emits `unset_delegate` governance event
- **Effects**: Records revocation in history for compliance
- **Use Case**: Cold storage account revokes hot wallet authority

```rust
// Example: Cold storage revokes delegation
PredictifyHybrid::unset_delegate(
    env,
    cold_storage_address,
    hot_wallet_address,
)?;
```

**`revoke_delegation(delegator: Address, delegate: Address) -> Result<(), Error>`**

Allows a delegate to self-revoke their voting authority. This is useful for immediate security response if the delegate's keys are compromised.

- **Authorization**: Requires `delegate.require_auth()`
- **Events**: Emits `unset_delegate` governance event with self-revocation marker
- **Effects**: Records revocation with `self_rev` reason
- **Use Case**: Compromised hot wallet disables its own authority

```rust
// Example: Hot wallet self-revokes due to compromise
PredictifyHybrid::revoke_delegation(
    env,
    cold_storage_address,
    hot_wallet_address,  // delegate self-revokes
)?;
```

#### Read-Only Operations (No Authorization Required)

**`has_delegation(delegator: Address, delegate: Address) -> bool`**

Check if a delegation exists.

```rust
if PredictifyHybrid::has_delegation(env, delegator, delegate) {
    println!("Delegation is active");
}
```

**`get_active_delegate(delegator: Address) -> Option<Address>`**

Get the currently active delegate for a delegator.

```rust
match PredictifyHybrid::get_active_delegate(env, delegator) {
    Some(delegate) => println!("Currently delegating to: {}", delegate),
    None => println!("No active delegation"),
}
```

**`get_delegations_by_delegator(delegator: Address) -> Result<Vec<Delegation>, Error>`**

Get all delegations for a delegator (typically 0 or 1).

```rust
let delegations = PredictifyHybrid::get_delegations_by_delegator(env, delegator)?;
for dlg in delegations {
    println!("Delegated since: {}", dlg.activated_at);
}
```

**`get_revocation_history(delegator: Address, delegate: Address) -> Result<Vec<Revocation>, Error>`**

Get the audit trail of revocations for compliance purposes.

```rust
let history = PredictifyHybrid::get_revocation_history(env, delegator, delegate)?;
println!("Revocations: {}", history.len());
```

## Voting Integration

When a vote is cast, the voting system must check whether the voter has delegated their rights:

### Voting Authority Resolution

When validating a vote, the system performs the following checks in order:

1. **Direct Vote**: If `user` has not delegated away voting rights
   - The vote is cast directly as `user`
   - Requires `user.require_auth()`

2. **Delegated Vote**: If `user` has delegated to `delegate`
   - The vote can be cast by either:
     - The `user` themselves (delegator can vote)
     - The `delegate` on behalf of `user`
   - Both require appropriate authorization

3. **Voted On Behalf**: If another address has delegated to `user`
   - `user` can vote on behalf of that delegator
   - The vote authority is attributed to the delegator, not the delegate

### Vote Attribution

Voting stakes and earnings are always attributed to the delegator (the account with voting rights), not the delegate (the account exercising those rights). This ensures:

- Voting power remains with the cold storage account
- Winnings accrue to the cold storage account
- Voting analytics credit the correct account

## Security Best Practices

### For Institutional Participants

#### 1. Cold Storage Account Setup

- Use a multi-signature or hardware wallet for the cold storage account
- Configure timelock delays for delegation changes (if your process allows)
- Store cold storage credentials in a highly secure environment
- Rotate access regularly

```rust
// Cold storage delegates voting authority
let cold_storage = Address::from_string("G..."); // Multi-sig account
let hot_wallet = Address::from_string("G...");   // Single hot wallet

PredictifyHybrid::delegate_votes(env, cold_storage, hot_wallet)?;
```

#### 2. Hot Wallet Setup

- Use a dedicated hot wallet for voting operations only
- Configure appropriate spending limits if possible
- Rotate the hot wallet regularly
- Monitor for unusual voting patterns

```rust
// Hot wallet votes on behalf of cold storage
let vote_result = PredictifyHybrid::vote_on_market(
    env,
    hot_wallet,          // Authority comes from this account
    market_id,
    outcome,
    stake,
)?;
```

#### 3. Revocation Procedures

- Establish clear procedures for revoking delegations during security incidents
- Document and communicate the revocation process to all stakeholders
- Test revocation procedures in non-production environments first

```rust
// Emergency: Revoke delegation if hot wallet is compromised
if breach_detected {
    PredictifyHybrid::unset_delegate(
        env,
        cold_storage,
        compromised_hot_wallet,
    )?;
    // Immediately deploy new hot wallet
    PredictifyHybrid::delegate_votes(
        env,
        cold_storage,
        new_hot_wallet,
    )?;
}
```

#### 4. Monitoring and Auditing

- Monitor delegation changes via `set_delegate` and `unset_delegate` events
- Regularly query `get_revocation_history()` for audit purposes
- Verify active delegates match expected configuration
- Track voting activity attributed to your cold storage account

```rust
// Verify current delegation configuration
let active = PredictifyHybrid::get_active_delegate(env, cold_storage)?;
assert_eq!(active, Some(expected_hot_wallet), "Delegation mismatch!");

// Audit revocation history
let history = PredictifyHybrid::get_revocation_history(
    env,
    cold_storage,
    old_hot_wallet,
)?;
println!("Previous delegate disabled {} times", history.len());
```

### Security Constraints

#### Single Active Delegate

Each delegator can have only one active delegate. Attempting to delegate to a second address automatically replaces the first delegation:

```rust
// First delegation
PredictifyHybrid::delegate_votes(env, cold_storage, wallet_a)?;

// This replaces the first delegation
PredictifyHybrid::delegate_votes(env, cold_storage, wallet_b)?;

// wallet_a is no longer authorized
assert!(!PredictifyHybrid::has_delegation(env, cold_storage, wallet_a));
```

#### Circular Delegation Prevention

The system prevents direct circular delegation (A→B→A):

```rust
// A → B
PredictifyHybrid::delegate_votes(env, addr_a, addr_b)?;

// B → A fails (circular)
let result = PredictifyHybrid::delegate_votes(env, addr_b, addr_a);
assert!(result.is_err(), "Circular delegation prevented");
```

**Note**: Longer cycles (A→B→C→A) are not prevented to optimize for gas efficiency. Institutions should establish governance policies to prevent policy violations.

#### No Self-Delegation

An account cannot delegate to itself:

```rust
let result = PredictifyHybrid::delegate_votes(env, account, account);
assert!(result.is_err(), "Self-delegation prevented");
```

## Error Handling

### Error Codes (550-558)

- **550 `DelegationNotFound`**: Delegation doesn't exist
- **551 `DelegationSelfDelegation`**: Attempt to delegate to self
- **552 `DelegationCircular`**: Circular delegation would result
- **553 `DelegationAlreadyExists`**: Delegator already has delegation
- **554 `DelegationMaxExceeded`**: Too many delegations from/to account
- **555 `DelegationCooldownActive`**: Revocation cooldown period active
- **556 `DelegationUnauthorized`**: Unauthorized delegation operation
- **557 `DelegationExpired`**: Delegation has expired
- **558 `DelegationInvalidParams`**: Invalid delegation parameters

### Recovery Strategies

| Error | Cause | Recovery |
|-------|-------|----------|
| `DelegationNotFound` | No active delegation | Establish new delegation or use direct voting |
| `DelegationSelfDelegation` | Attempted self-delegation | Choose a different delegate address |
| `DelegationCircular` | Would create circular chain | Change the target or revoke existing delegation first |
| `DelegationUnauthorized` | Wrong account authorized | Ensure delegator signs the transaction |

## Events

The delegation system emits two governance events:

### `set_delegate` Event

Fired when a delegation is established or modified.

```rust
env.events().publish(
    ("gov_dlgset",),
    (delegator, delegate),
);
```

**Use Cases**:
- Monitoring delegation changes
- Compliance reporting
- Audit trails

### `unset_delegate` Event

Fired when a delegation is revoked (either by delegator or delegate).

```rust
env.events().publish(
    ("gov_dlguns",),
    (delegator, delegate),
);
```

**Use Cases**:
- Detecting security incidents
- Compliance reporting
- Audit trails

## Implementation Checklist

- [ ] Deploy cold storage account (multi-sig or hardware wallet)
- [ ] Deploy hot wallet for voting operations
- [ ] Establish delegation: cold_storage → hot_wallet
- [ ] Verify delegation with `has_delegation()` query
- [ ] Monitor `set_delegate` and `unset_delegate` events
- [ ] Document delegation procedures for your team
- [ ] Test revocation procedures in testnet
- [ ] Establish security monitoring for voting activity
- [ ] Plan regular hot wallet rotation
- [ ] Document and communicate delegation policy

## Frequently Asked Questions

### Q: Can I have multiple delegates?

A: No, each delegator can have only one active delegate. If you need to rotate delegates, revoke the old one and establish a new one.

### Q: What happens if I delegate and then vote directly?

A: Voting integration allows both the delegator and delegate to vote on behalf of the delegator's account. Both operations are valid and contribute to the voting stake.

### Q: Can I delegate with a timelock delay?

A: The current implementation does not include timelock delays. Consider implementing this in your application layer if needed for additional security.

### Q: How do I know if my delegation is still active?

A: Query `has_delegation()` to verify, or call `get_active_delegate()` to see the current delegate.

### Q: Can I recover if my hot wallet is compromised?

A: Yes, either:
1. Use your cold storage account to revoke via `unset_delegate()`
2. Use the compromised account to self-revoke via `revoke_delegation()` (if keys not fully exposed)
3. Deploy a new hot wallet and delegate to it

### Q: Are delegation changes auditable?

A: Yes, use `get_revocation_history()` to see all previous delegations and changes. The events `set_delegate` and `unset_delegate` are also emitted for off-chain tracking.

## Deployment Considerations

### Gas Efficiency

- Delegation queries are read-only and inexpensive
- Establishing and revoking delegations use persistent storage
- Circular delegation check is O(1) and inexpensive
- Consider caching `get_active_delegate()` result locally if checking frequently

### Scalability

- Each delegator can have one active delegation
- Revocation history grows with repeated revocations
- Storage is persistent and not garbage-collected
- Monitor storage growth in high-frequency scenarios

### Network Recommendations

- Use testnet to validate procedures before mainnet deployment
- Start with a small pilot group before wide rollout
- Monitor voting activity patterns after delegation deployment
- Establish incident response procedures for delegation changes

## Further Reading

- [Soroban Smart Contract Documentation](https://soroban.stellar.org/)
- [Stellar Account Types](https://developers.stellar.org/docs/learn/fundamentals/account-management)
- [Multi-Signature Accounts](https://developers.stellar.org/docs/learn/fundamentals/account-management/multisig)
