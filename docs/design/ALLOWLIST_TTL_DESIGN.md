# Allowlist TTL (Time-To-Live) Design

## Overview

This document specifies a time-to-live (TTL) mechanism for allowlist entries to mitigate the blast radius from compromised private keys. Currently, once an address is added to an allowlist, it retains access indefinitely. This design introduces automatic expiration and re-verification to limit the window of privilege exploitation.

## Problem Statement

**Current State:**
- Allowlist entries are permanent once added
- A compromised key gains indefinite privileged access
- No mechanism to revoke access without contract upgrades or manual removal
- Blast radius from key compromise is unbounded

**Risk Impact:**
- Long-term privilege escalation attacks
- Difficult incident response (no automatic containment)
- No incentive for security hygiene (re-verification)

## Proposed Solution

### 1. TTL-Based Expiration

Each allowlist entry includes:
- `address`: The whitelisted address
- `added_at`: Timestamp when entry was created
- `expires_at`: Calculated as `added_at + TTL_DURATION`
- `verified_at`: Most recent re-verification timestamp

**TTL Duration Options:**
- 90 days: Moderate security, acceptable UX
- 30 days: High security, requires frequent re-verification
- Custom per-entry: Maximum flexibility, operational complexity

### 2. Re-Verification Mechanism

Before expiration, addresses must submit valid re-authorization to extend access:

```rust
pub fn renew_allowlist_entry(
    env: Env,
    address: Address,
    proof: AuthProof,  // Signature or other proof of authorization
) -> Result<(), AllowlistError> {
    // 1. Verify proof is valid
    // 2. Check if entry exists
    // 3. Update verified_at to current time
    // 4. Extend expires_at by another TTL period
    // 5. Emit event for audit trail
}
```

### 3. Access Control During Expiration

Expired entries are treated as unauthorized:

```rust
fn is_allowlist_valid(env: &Env, address: &Address) -> bool {
    if let Some(entry) = get_allowlist_entry(address) {
        entry.expires_at > env.ledger().timestamp()
    } else {
        false
    }
}
```

## Implementation Architecture

### Data Structure

```rust
pub struct AllowlistEntry {
    pub address: Address,
    pub added_at: u64,           // Ledger timestamp
    pub expires_at: u64,         // added_at + TTL_DURATION
    pub verified_at: u64,        // Most recent re-verification
    pub verification_count: u32, // For analytics/monitoring
}

pub const TTL_DURATION_SECONDS: u64 = 90 * 24 * 60 * 60; // 90 days
pub const RENEWAL_BUFFER_SECONDS: u64 = 7 * 24 * 60 * 60; // Can renew 7 days before expiry
```

### Contract Entrypoints

#### add_allowlist_entry
- **Behavior**: Add address with TTL = now + TTL_DURATION
- **Auth**: Requires admin
- **Events**: `AllowlistEntryAdded { address, expires_at }`

#### renew_allowlist_entry
- **Behavior**: Extend expiration if proof is valid
- **Auth**: Address owner (self-renewing) OR admin
- **Events**: `AllowlistEntryRenewed { address, new_expires_at }`
- **Error**: Returns error if entry expired (must be re-added)

#### is_allowlisted
- **Behavior**: Check if address is in valid allowlist
- **Auth**: Public read
- **Returns**: `Result<bool, AllowlistError>`; returns `Ok(false)` when the
  address is absent, `AllowlistExpired` when the allowlist storage entry has
  expired but its ID remains registered, and `AllowlistNotFound` when the ID is
  absent or deleted.

#### list_expiring_soon
- **Behavior**: Return addresses expiring within N days (e.g., 14 days)
- **Auth**: Public read (for UX alerts)
- **Returns**: `Vec<(Address, u64)>` with expires_at timestamps

#### remove_allowlist_entry (existing)
- **No changes**: Admin can always manually remove entries

### Off-Chain Integration

**Recommended flow for long-term key holders:**

1. User receives notification at `expires_at - 14 days`
2. User submits renewal transaction before `expires_at - 7 days`
3. If missed, user must go through full add-back process (likely requires admin approval)

## Security Considerations

### Threat Model

**Mitigated Threats:**
- Compromise at T0 has limited impact window (expires at T0 + 90 days)
- Attacker cannot exploit stale compromised credentials indefinitely
- Encourages security hygiene (forced re-verification)

**Remaining Threats:**
- Compromise detected after expiry already occurred (periodic monitoring needed)
- Compromise of renewal/re-authorization mechanism
- Replay attacks on renewal proofs (mitigate with nonce/timestamp in proof)

### Edge Cases

1. **Entry expires mid-transaction**: Entry should be checked at transaction start; expiry during execution is acceptable
2. **Clock skew**: Use consensus timestamp (env.ledger().timestamp()), not local time
3. **Renewal race conditions**: Use atomic compare-and-swap or sequential numbering

## Testing Strategy

### Unit Tests
- [ ] TTL calculation accuracy
- [ ] Renewal extends expiry correctly
- [ ] Expired entries rejected in auth checks
- [ ] Premature renewal blocked (buffer window)

### Integration Tests
- [ ] Full lifecycle: add → renew → expire → re-add
- [ ] Concurrent renewal attempts
- [ ] Admin removal of entries mid-lifecycle
- [ ] Ledger timestamp boundary conditions

### Auth Boundary Tests
- [ ] Non-owner cannot renew another's entry
- [ ] Admin can renew on behalf
- [ ] Expired entry cannot be used regardless of original admin auth

## Deployment Considerations

### Migration Path

1. **Phase 1**: Deploy new contract with TTL logic
   - New entries use TTL
   - Existing entries treated as "perpetual" or grandfathered in
2. **Phase 2**: Notify all allowlisted addresses to renew before cutover
3. **Phase 3**: Enforce TTL on all entries (including grandfathered)

### Monitoring & Alerting

- Track renewal rate per entry
- Alert on entries approaching expiry
- Log all re-verification attempts (success/failure)
- Dashboard showing "healthy" vs "expiring" allowlist

## Configuration Options

```rust
pub struct AllowlistConfig {
    pub ttl_duration_seconds: u64,      // Default: 90 days
    pub renewal_buffer_seconds: u64,    // Default: 7 days before expiry
    pub max_renewal_window: u64,        // How far ahead can be renewed? (Default: until expiry)
    pub require_proof_on_renewal: bool, // Does renewal require auth proof?
}
```

## Alternatives Considered

### 1. No TTL (Current State)
- **Pros**: Simplest, no UX friction
- **Cons**: Unbounded blast radius from compromise
- **Decision**: Rejected due to security risk

### 2. Hard Expiry (No Renewal)
- **Pros**: Simple, deterministic
- **Cons**: Users must be re-added (admin overhead), no self-service renewal
- **Decision**: Less practical for operational maturity

### 3. Rolling Window (Auto-Renew on Use)
- **Pros**: No manual renewal needed
- **Cons**: Compromised key could extend indefinitely by using account
- **Decision**: Doesn't achieve blast radius reduction goal

### 4. Periodic Rotation (Replace with New Address)
- **Pros**: Enforces key rotation discipline
- **Cons**: Operational complexity, users must pre-stage new keys
- **Decision**: More aggressive than needed; TTL + renewal balances security and UX

## Success Metrics

- [ ] TTL mechanism deployed and tested in testnet
- [ ] 95%+ allowlisted addresses renew proactively before expiry
- [ ] No legitimate access interrupted by expiration
- [ ] Renewal transaction cost < 1% of typical transaction value
- [ ] Clear audit trail for all TTL events

## Timeline

- Week 1: Implement TTL data structure and core logic
- Week 2: Implement renewal mechanism and tests
- Week 3: Integration testing and documentation
- Week 4: Testnet deployment and monitoring
- Week 5+: Gradual mainnet rollout with phased migration
