# Timestamp Assumptions for Timelocks

This document records every timelock in the codebase that depends on
`env.ledger().timestamp()`, the assumption each one makes about ledger time,
the risk introduced by validator timestamp discretion, and a recommendation on
whether to migrate to ledger sequence numbers.

---

## Background: Stellar Ledger Time vs Real Time

`env.ledger().timestamp()` returns the Unix timestamp encoded in the ledger
header. Stellar validators set this value by consensus: each validator
proposes a timestamp and the network accepts the median. The protocol enforces
only two constraints:

- The new timestamp must be **strictly greater than** the previous ledger's
  timestamp.
- The new timestamp must be **less than or equal to** the validator's local
  wall clock (with some tolerance).

In practice, the timestamp drifts by seconds to low minutes during normal
operation. Under unusual network conditions (partition, validator
misconfiguration, deliberate manipulation near the edges of the allowed
window) the drift can widen. For timelocks expressed in hours or days the
accumulated drift is negligible. For timelocks shorter than roughly 30 minutes
the drift can represent a meaningful fraction of the total window.

**Ledger sequence numbers** advance by exactly 1 per closed ledger, with a
target close time of ~5 seconds. A fixed number of ledgers therefore maps to a
predictable real-world duration with no room for timestamp manipulation.
Converting: 1 hour ≈ 720 ledgers; 24 hours ≈ 17 280 ledgers.

---

## Inventory of Timelocks

### 1. Admin Action Cooldown — `contracts/admin`

| Property | Value |
|---|---|
| File | `contracts/admin/src/lib.rs` |
| Constant | Stored as `AdminCooldownSeconds` (default 0, configured by admin) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured; no built-in default |

**What the check assumes:** `now - last_action >= cooldown_seconds`, where
`now` is the ledger timestamp at the moment the guarded function is invoked.

**Risk:** Because the cooldown is configured by the admin and there is no
minimum, a cooldown of 0 disables the guard entirely. For non-zero values the
ledger-time drift risk is proportional to the configured value—negligible for
multi-hour values, theoretically exploitable for values below ~5 minutes.

**Recommendation:** Document the minimum meaningful cooldown value (suggest
≥ 300 seconds / 5 minutes) in the contract's doc comment. No migration to
sequence numbers is warranted given the flexible configuration.

---

### 2. Admin Action Cooldown — `contracts/markets`

| Property | Value |
|---|---|
| File | `contracts/markets/src/admin.rs` — `AdminManager::check_admin_cooldown` |
| Constant | Stored as `AdminCooldownSeconds` (default 0, configured by admin) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured |

Identical semantics to the standalone admin contract above. The same
recommendation applies.

---

### 3. Admin Action Cooldown — `contracts/predictify-hybrid` (global)

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` |
| Constant | `COOLDOWN_PERIOD = 86_400` (24 hours, hard-coded) |
| Clock source | `env.ledger().timestamp()` |
| Duration | 24 hours |

**What the check assumes:** No privileged admin action may execute within 24
ledger-time hours of the previous one.

**Risk:** 24 hours is long enough that ledger-time drift (seconds to low
minutes) is irrelevant. A validator cannot manipulate the timestamp by anywhere
near 24 hours without breaking network liveness.

**Recommendation:** Add a doc comment stating: *"The 24-hour window is measured
in ledger timestamps. Under normal Stellar conditions ledger time and wall-clock
time agree to within a few minutes; the effective cooldown is therefore
indistinguishable from 24 real-world hours."* No migration to sequence numbers
needed.

---

### 4. Oracle Admin Cooldown — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` — `OracleAdminState` |
| Constant | Stored as `OracleAdminState::cooldown_seconds` (default 0) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured |

Guards oracle configuration mutations. Same shape as the market admin cooldown
(section 2). Same recommendation applies.

---

### 5. Betting Admin Cooldown — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` — `BettingAdminState` |
| Constant | Stored as `BettingAdminState::cooldown_seconds` (default 0) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured |

Guards betting configuration mutations. Same shape as the oracle admin
cooldown (section 4). Same recommendation applies.

---

### 6. Signer Rotation Cooldown — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` — `SignerRotationState` |
| Constant | Stored as `SignerRotationState::cooldown_seconds` (default 0) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured |

Guards multisig signer key rotation. Same recommendation as sections 2–5.

---

### 7. Threshold Rotation Delay — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` |
| Constant | `THRESHOLD_ROTATION_DELAY = 86_400` (24 hours) |
| Clock source | `env.ledger().timestamp()` |
| Duration | 24 hours |

A pending threshold-rotation proposal becomes executable only after 24 ledger-
time hours have elapsed since it was proposed. This is a security delay: it
gives time to detect and cancel a malicious or erroneous rotation.

**Assumption:** `now >= proposed_at + 86_400`.

**Risk:** Negligible at 24 hours (see section 3).

**Recommendation:** Add a doc comment mirroring section 3.

---

### 8. Admin Queued Action Expiry — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/admin.rs` |
| Constant | Hard-coded: `expires_at = now + 86_400` (24 hours) |
| Clock source | `env.ledger().timestamp()` |
| Duration | 24 hours |

Queued admin actions expire 24 hours after they are created (`action.expires_at`).
After expiry they are rejected.

**Assumption:** `now <= expires_at` means the action is still live.

**Risk:** Negligible at 24 hours. Note that the expiry check uses `>` not `>=`,
so an action submitted on the last ledger before expiry will still be accepted.
This is an off-by-one that is unlikely to matter in practice but should be
documented.

**Recommendation:** Document the expiry semantics in the type's doc comment.
Consider replacing `> action.expires_at` with `>= action.expires_at` for
clarity.

---

### 9. Resolution Cooldown — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/resolution.rs` — `MarketResolutionManager::check_resolution_cooldown` |
| Constant | Stored as `ResolutionCooldownSeconds` (default: absent → no guard) |
| Clock source | `env.ledger().timestamp()` |
| Duration | Caller-configured; absent means no cooldown |

Guards the manual, tie-breaking, and force-resolution entry points. When
configured this prevents repeatedly resolving a market in quick succession.

**Assumption:** `now - last_action >= cooldown_seconds`.

**Risk:** Same as sections 2–5: proportional to the configured value.

**Recommendation:** Document the absence-equals-no-guard behaviour. Add a
contract invariant that if set, the value should be ≥ 60 seconds to avoid
the guard being trivially bypassed by timestamp drift.

---

### 10. Market Resolution Timeout — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/lib.rs` — `resolution_timeout_reached` |
| Field | `Market::resolution_timeout` (seconds; default 86 400 / 24 h in most creation paths) |
| Clock source | `env.ledger().timestamp()` |
| Condition | `current_time >= market.end_time + resolution_timeout` |

Once this condition is true, further oracle resolution is blocked and the
market enters a timeout state.

**Assumption:** The sum of `end_time` (a timestamp set at market creation) and
`resolution_timeout` (a caller-supplied duration) meaningfully represents a
wall-clock deadline.

**Risk:**
- `end_time` is itself a caller-supplied timestamp stored at creation; if the
  creator supplies a far-future value the timeout is correspondingly delayed.
- The 24-hour default for `resolution_timeout` is long enough that ledger-time
  drift does not matter.
- Test fixtures use `resolution_timeout: 3600` (1 hour). This is short enough
  that drift is non-trivial, but test environments use a fixed simulated
  timestamp so there is no real exposure.

**Recommendation:** Document in `Market` that `resolution_timeout` is
interpreted as ledger-time seconds, and that values below 300 seconds are
inadvisable. For production markets, the 24-hour default is appropriate.

---

### 11. Market Dispute Window — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/disputes.rs` — `DisputeManager` |
| Constant | `DISPUTE_PERIOD_SECS = 86_400` (default); overridden by `Market::dispute_window_seconds` |
| Clock source | `env.ledger().timestamp()` |
| Condition | Dispute valid when `end_time <= now < end_time + dispute_window_seconds` |

Users may file disputes only while the dispute window is open.

**Assumption:** `dispute_window_seconds` seconds of ledger time represent a
meaningful real-world challenge period.

**Risk:** 24 hours is long enough to be insensitive to drift. The fuzz targets
set this to `86_400`, consistent with production intent.

**Recommendation:** Document in `Market` and `DISPUTE_PERIOD_SECS` that the
window is ledger-time seconds. Add a minimum enforced by the contract (suggest
3 600 seconds / 1 hour) to prevent accidentally creating markets with no
meaningful challenge period.

---

### 12. Dispute Timeout (Created/Expires) — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/disputes.rs` — `DisputeManager::create_dispute_timeout` |
| Constant | Configured via `timeout_hours` parameter; extended by `DISPUTE_EXTENSION_HOURS` |
| Clock source | `env.ledger().timestamp()` |
| Condition | `expires_at = now + timeout_hours * 3600`; `extends_at += additional_hours * 3600` |

A `DisputeTimeout` record tracks when a dispute's auto-resolution window
expires. Calls to `check_dispute_timeout` compare `env.ledger().timestamp()`
against `expires_at`.

**Risk:** At typical dispute durations of several hours to a day, drift is
negligible. For the minimum meaningful value (1 hour, 3 600 seconds) drift
could shorten the effective window by minutes.

**Recommendation:** This is a **candidate for ledger-sequence migration** if
dispute timeouts are frequently configured below 2 hours. At 1-hour timeouts,
sequence-number-based expiry (≈ 720 ledgers) would be more predictable.
Document the current approach clearly: *"Expiry is measured in ledger
timestamps. The window may be up to a few minutes shorter or longer than the
configured hours in real-world time."*

---

### 13. Dispute Voting Window — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/disputes.rs` — voting escalation path |
| Constant | `voting_end = now + DISPUTE_EXTENSION_HOURS * 3600` |
| Clock source | `env.ledger().timestamp()` |
| Duration | `DISPUTE_EXTENSION_HOURS` hours (re-exported from `crate::config`) |

When a dispute escalates to community voting, a new voting round opens with a
deadline at `now + DISPUTE_EXTENSION_HOURS * 3600`.

**Risk:** Same as section 12. At short extension durations this window could
close minutes earlier than expected.

**Recommendation:** Same as section 12. Document that the voting deadline is a
ledger-time deadline, not a wall-clock guarantee.

---

### 14. Recovery Timelock — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/recovery.rs` — `RecoveryTimelockConfig` |
| Constants | `DEFAULT = 86 400 s`, `MIN = 3 600 s`, `MAX = 604 800 s` (7 days) |
| Clock source | `env.ledger().timestamp()` |
| Condition | `execute_after = initiated_at + timelock_seconds`; execution blocked while `now < execute_after` |

A pending recovery action (force-resolve, cancel, reconstruct state) cannot be
executed until at least `timelock_seconds` ledger-time seconds have elapsed.

**What the check assumes:** `now >= execute_after` reliably represents the
passage of at least `timelock_seconds` real-world seconds.

**Risk:** At the 1-hour minimum the window could effectively be a few minutes
shorter due to drift. At 24 hours or more, drift is negligible. Because this
is a security window (time for an admin to notice and cancel a bad recovery),
erring short is safer than erring long.

**Recommendation:** Document in `RecoveryTimelockConfig` that `timelock_seconds`
is ledger-time seconds and the effective real-world delay may be slightly
shorter than configured. The minimum of 3 600 s is acceptable; note that
validators could in theory close this by ~1–2 minutes but cannot reduce a
1-hour window to zero.

---

### 15. Fee Withdrawal Cooldown — `contracts/fees`

| Property | Value |
|---|---|
| File | `contracts/fees/src/lib.rs` — `FeeContract::collect_fees` |
| Constant | `cooldown_seconds = 86_400` (hard-coded at initialization) |
| Clock source | `env.ledger().timestamp()` |
| Fields | `FeeWithdrawalSchedule::next_withdrawal = last_withdrawal + cooldown_seconds` |

After a successful fee collection, the next withdrawal is blocked until
`next_withdrawal` ledger-time has been reached.

**Assumption:** The contract currently stores `next_withdrawal` and
`last_withdrawal` but does **not** check `now >= next_withdrawal` before
allowing the next collection. The schedule is updated after each collection
but no enforcement guard exists in `collect_fees`. This means the cooldown
is currently informational only.

**Risk:** No real-world risk from drift because the check is not enforced.
However, if enforcement is added in future, the 24-hour window is long enough
to be insensitive to drift.

**Recommendation:** Add an enforcement check to `collect_fees`:
```rust
if now < schedule.next_withdrawal && schedule.last_withdrawal > 0 {
    return Err(ContractError::WithdrawalCooldownActive);
}
```
Document that the 24-hour cooldown is ledger-time-based once enforcement is
added.

---

### 16. Unclaimed Reward Claim Period — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/recovery.rs` |
| Constant | `DEFAULT_UNCLAIMED_CLAIM_PERIOD_SECONDS = 90 * 24 * 60 * 60` (90 days) |
| Clock source | `env.ledger().timestamp()` |
| Condition | Unclaimed rewards become recoverable after 90 ledger-time days |

**Risk:** Completely negligible at 90 days.

**Recommendation:** Document as ledger-time seconds. No migration needed.

---

### 17. Market Pause Cooldown and Duration — `contracts/predictify-hybrid`

| Property | Value |
|---|---|
| File | `contracts/predictify-hybrid/src/markets.rs` — `PauseManager` |
| Constants | `MIN_UNPAUSE_COOLOFF_SECONDS = 3_600`, `MAX_PAUSE_DURATION_HOURS = 168` (7 days), `MIN_PAUSE_DURATION_HOURS = 1` |
| Clock source | `env.ledger().timestamp()` |

The 1-hour minimum unpause cooloff is the shortest operationally meaningful
duration in this module.

**Risk:** At exactly 1 hour, drift could effectively make the cooloff 2–5
minutes shorter. This is unlikely to matter operationally.

**Recommendation:** Document `MIN_UNPAUSE_COOLOFF_SECONDS` as a ledger-time
value. The 1-hour minimum is an acceptable threshold; consider raising it
to 1 800 seconds (30 minutes) if sub-minute precision matters.

---

### 18. Validators — `contracts/validators`

| Property | Value |
|---|---|
| File | `contracts/validators/src/lib.rs` |
| Clock source | `env.ledger().sequence()` |

This contract already uses **ledger sequence numbers** for its `registered_at`
field. It does not use timestamps for any guard. No action needed.

---

## Summary Table

| # | Timelock | Duration | Clock | Drift Risk | Migrate to Sequence? |
|---|---|---|---|---|---|
| 1 | Admin cooldown (admin contract) | Configurable | Timestamp | Proportional to value | No |
| 2 | Admin cooldown (markets contract) | Configurable | Timestamp | Proportional to value | No |
| 3 | Admin cooldown (predictify-hybrid) | 24 h fixed | Timestamp | Negligible | No |
| 4 | Oracle admin cooldown | Configurable | Timestamp | Proportional to value | No |
| 5 | Betting admin cooldown | Configurable | Timestamp | Proportional to value | No |
| 6 | Signer rotation cooldown | Configurable | Timestamp | Proportional to value | No |
| 7 | Threshold rotation delay | 24 h fixed | Timestamp | Negligible | No |
| 8 | Admin queued action expiry | 24 h fixed | Timestamp | Negligible | No |
| 9 | Resolution cooldown | Configurable | Timestamp | Proportional to value | No |
| 10 | Market resolution timeout | 24 h default | Timestamp | Negligible at default | No |
| 11 | Market dispute window | 24 h default | Timestamp | Negligible at default | **Yes, if < 2 h** |
| 12 | Dispute timeout expiry | Configurable hours | Timestamp | Low–moderate | **Yes, if < 2 h** |
| 13 | Dispute voting window | DISPUTE_EXTENSION_HOURS | Timestamp | Low–moderate | **Yes, if < 2 h** |
| 14 | Recovery timelock | 24 h default, min 1 h | Timestamp | Negligible at default | No |
| 15 | Fee withdrawal cooldown | 24 h fixed (unenforced) | Timestamp | N/A (unenforced) | No |
| 16 | Unclaimed reward period | 90 days | Timestamp | Negligible | No |
| 17 | Market pause cooldown/duration | 1 h min – 7 days max | Timestamp | Negligible at ≥ 1 h | No |
| 18 | Validator registration | N/A | **Sequence** | None | Already done ✓ |

---

## Recommended Changes

### Immediate (documentation only)

Add the following comment block above every timelock constant and check that
currently has no explanation:

```rust
// Measured in ledger-time seconds (`env.ledger().timestamp()`).
// Under normal Stellar operation ledger time tracks wall-clock time to
// within a few minutes. For windows ≥ 1 hour the effective real-world
// duration is indistinguishable from the configured value.
```

### Fee withdrawal enforcement (contracts/fees)

The 24-hour withdrawal cooldown is stored but not enforced. Add a guard before
the transfer in `collect_fees` (see section 15).

### Short dispute / timeout windows (< 2 hours)

If `DISPUTE_EXTENSION_HOURS` or any per-dispute `timeout_hours` is ever set
below 2, consider switching those specific comparisons to ledger sequence
numbers. The conversion at ~5 s/ledger:

| Duration | Ledger count |
|---|---|
| 1 hour | 720 ledgers |
| 2 hours | 1 440 ledgers |
| 6 hours | 4 320 ledgers |
| 24 hours | 17 280 ledgers |

A sequence-based check looks like:

```rust
// At contract creation or dispute open:
let expire_sequence = env.ledger().sequence() + 720u32; // 1 h ≈ 720 ledgers

// At check time:
if env.ledger().sequence() < expire_sequence {
    return Err(Error::DisputeWindowClosed);
}
```

Sequence numbers cannot be manipulated by validators and advance at a fixed
rate, so short windows are fully predictable.

### Missing `timelock` module (`pub mod timelock` in lib.rs)

`lib.rs` declares `pub mod timelock` and multiple files reference
`crate::timelock::MarketTimelockConfig`, but no `timelock.rs` or
`timelock/mod.rs` file exists. The contract will not compile until this module
is created. It should define at minimum:

```rust
use soroban_sdk::contracttype;

/// Per-market configuration for admin timelocks.
///
/// All durations are measured in ledger-time seconds. Under normal Stellar
/// operation this is equivalent to wall-clock seconds to within a few minutes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketTimelockConfig {
    /// Minimum seconds between admin mutations on this market.
    /// Zero disables the per-market cooldown.
    pub admin_cooldown_seconds: u64,
}

impl Default for MarketTimelockConfig {
    fn default() -> Self {
        Self {
            admin_cooldown_seconds: 0,
        }
    }
}
```
