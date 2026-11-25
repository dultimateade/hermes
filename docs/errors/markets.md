# Markets Error Catalog

This document describes the `ContractError` variants for the markets smart contract.

## Error Codes

| Code | Name | Description |
| ---- | ---- | ----------- |
| `1` | `Unauthorized` | Action is unauthorized; typically thrown when an admin action is invoked by a non-admin. |
| `2` | `MarketNotFound` | Market not found; thrown when querying or interacting with a non-existent market. |
| `3` | `MarketClosed` | Market is closed; thrown when trying to interact with a market that has already ended. |
| `4` | `MarketAlreadyResolved` | Market already resolved; thrown when attempting to resolve a market more than once. |
| `5` | `MarketNotResolved` | Market not resolved; thrown when attempting to claim winnings before resolution. |
| `6` | `InvalidOutcome` | Invalid outcome; thrown when the provided outcome is not supported by the market. |
| `7` | `InvalidConfig` | Invalid configuration; thrown when market setup parameters are out of bounds. |
| `8` | `Overflow` | Overflow; thrown when math operations overflow, preventing unsafe state changes. |
| `9` | `StakeTooSmall` | Stake too small; thrown when a deposit or bet is below the minimum threshold. |
| `10` | `InvalidState` | Invalid State; thrown when a generic state transition fails, or when an invariant (e.g. LP reward index vs. reward debt) is violated. |
| `15` | `AlreadyClaimed` | Already claimed; thrown when a bet's winnings have already been claimed, including re-entrant claim attempts. |
| `19` | `InvalidPayoutCurve` | Invalid payout curve; thrown when a payout curve configuration is invalid (e.g. all outcome weights sum to zero). |

---

## `InvalidConfig` — Code `7`

### When returned

The `InvalidConfig` error is returned when market setup parameters are out of
bounds, including:

- Initialization on a network whose maximum storage TTL is smaller than the
  required payout-curve TTL (`MARKET_PAYOUT_CURVE_TTL_LEDGERS`).

### Recovery guidance

This error is **terminal** for the current call. The caller should:

1. Verify the configuration values against the documented bounds.
2. Deploy or initialize on a network with a sufficient maximum TTL, or adjust
   the configuration so it fits within the network limits.

Retrying with the same configuration will always fail.

---

## `InvalidState` — Code `10`

### When returned

The `InvalidState` error is returned when a generic state transition fails, or
when a stored invariant is violated. For example, `remove_liquidity` returns
this error when the global liquidity reward index is below a provider's stored
reward debt (e.g. after an emergency index reset), instead of underflowing.

### Recovery guidance

This error is **terminal** for the current call. The caller should:

1. Inspect the market's liquidity reward state (global index and reward debt).
2. Wait for the index to be reconciled by an admin, or contact support if the
   invariant remains violated.

Retrying without a state change will always fail.

---

## `AlreadyClaimed` — Code `15`

### When returned

The `AlreadyClaimed` error is returned by `claim_winnings` when the bet's
claimed flag is already set. The flag is written and re-verified inside each
payout iteration, so a re-entrant call injected via a token hook also receives
this error rather than a second payout.

### Recovery guidance

This error is **terminal** for the current call. The caller should:

1. Verify the bet has not already been claimed (check the claimed flag).
2. Do not retry; a successful claim cannot be repeated.

---

## `InvalidPayoutCurve` — Code `19`

### When returned

The `InvalidPayoutCurve` error is returned when a payout curve configuration is
invalid, including:

- All outcome weights sum to zero (would cause a division-by-zero at
  resolution).

### Recovery guidance

This error is **terminal** for the current call. The caller should:

1. Provide a curve where the total outcome weight is greater than zero.
2. Re-submit market creation with the corrected curve.

Retrying with the same curve will always fail.

---

## `MarketClosed` — Code `3`

### When returned

The `MarketClosed` error is returned whenever a caller attempts to interact
with a market that is no longer accepting operations:

- Placing a bet after the market's deadline.
- Attempting to vote or otherwise participate once the market has ended.

### Recovery guidance

This error is **terminal** for the current call. The caller should:

1. Verify the market ID is correct.
2. Check that the market's end time has not passed.
3. Look for a different market that is still active.

Retrying without a state change will always fail.

---

> See also: [`predictify-hybrid.md`](./predictify-hybrid.md) for the
> `MarketClosed = 102` error in the hybrid contract.
