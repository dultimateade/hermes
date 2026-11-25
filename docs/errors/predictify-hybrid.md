# Predictify-Hybrid Error Catalog

This document describes the `Error` variants for the `predictify-hybrid` smart
contract (module `err.rs`).

---

## `MarketClosed` — Code `102`

### Summary

The market is closed and cannot accept new bets or operations.

### When returned

| Entrypoint | Condition |
|---|---|
| `place_bet` / `place_bets` | `market.state != Active` **or** `current_time >= bet_deadline` |
| `cancel_bet` | `current_time >= market.end_time` |
| `resolve_market_manual` | `current_time < market.end_time` (market hasn't ended yet) |
| `resolve_market_with_ties` | `current_time < market.end_time` (market hasn't ended yet) |
| `fetch_oracle_result` | `current_time < market.end_time` |
| `MarketResolutionValidator::validate_market_for_resolution` | `market.is_active()` returns `true` |
| `OracleResolutionValidator::validate_market_for_oracle_resolution` | `current_time < market.end_time` |

### Recovery guidance

This is a **terminal** error for the current call.

- **For bettors**: The market's betting window is closed. Look for a different
  active market, or wait for a new one to open.
- **For admins resolving a market**: Wait until `market.end_time` has passed
  before calling `resolve_market_manual` or `resolve_market_with_ties`.

Retrying without changing the market state will always fail.

### Recoverability

`Abort` — the SDK recovery strategy is `RecoveryStrategy::Abort`.

---

## Oracle configuration rejections

Oracle configuration validation (`OracleConfig::validate`,
`OracleProvider::validate_for_market`, and
`MarketValidator::validate_oracle_config`) reports the specific field that was
rejected instead of a single generic error.

| Code | Name | Rejected when |
|---|---|---|
| `201` | `InvalidOracleConfig` | The configuration is the reserved "no oracle" sentinel. |
| `215` | `InvalidOracleProvider` | The provider is not supported on the current network. |
| `209` | `InvalidOracleFeed` | The feed ID is empty, is an unsupported Reflector asset, or its format is incompatible with the provider. |
| `303` | `InvalidThreshold` | The threshold is zero or negative. |
| `304` | `InvalidComparison` | The comparison operator is empty or not one of `gt`, `lt`, `eq`. |

These errors are terminal for the current call: fix the reported field and
resubmit. The SDK recovery strategy for the oracle-specific codes is
`RecoveryStrategy::NoRecovery`.

---

## Full Error Table

| Code | Name | Description |
|---|---|---|
| `100` | `Unauthorized` | User lacks the required permissions for this action. |
| `101` | `MarketNotFound` | Market ID is unknown or the market has been removed. |
| `102` | `MarketClosed` | Market has passed its deadline and cannot accept new operations. |
| `103` | `MarketResolved` | Market has already been resolved; no further betting allowed. |
| `104` | `MarketNotResolved` | Oracle resolution is still pending; winnings cannot be claimed yet. |
| `105` | `NothingToClaim` | The caller has no winnings to claim from this market. |
| `106` | `AlreadyClaimed` | Winnings have already been claimed; duplicate claims are rejected. |
| `107` | `InsufficientStake` | Bet amount is below the minimum required threshold. |
| `108` | `InvalidOutcome` | Chosen outcome does not exist in this market. |
| `109` | `AlreadyVoted` | User has already voted; only one vote per market is allowed. |
| `110` | `AlreadyBet` | User has already placed a bet on this market. |
| `111` | `BetsAlreadyPlaced` | Market parameters cannot be updated after bets have been placed. |
| `112` | `InsufficientBalance` | User balance is too low for the requested operation. |
| `200` | `OracleUnavailable` | External oracle is down or unreachable. |
| `201` | `InvalidOracleConfig` | Oracle configuration is missing or is the reserved "no oracle" sentinel. |
| `202` | `OracleStale` | Oracle data exceeds the freshness threshold. |
| `203` | `OracleNoConsensus` | Multiple oracle instances could not reach consensus. |
| `209` | `InvalidOracleFeed` | Oracle feed ID is empty, unsupported, or malformed for the provider. |
| `215` | `InvalidOracleProvider` | Oracle provider is not supported on the current network. |
| `300` | `InvalidQuestion` | Market question is empty or fails validation. |
| `301` | `InvalidOutcomes` | Outcome list is invalid (too few, duplicates, or empty). |
| `302` | `InvalidDuration` | Market duration is outside the allowed range (1–365 days). |
| `303` | `InvalidThreshold` | Oracle threshold is zero or negative. |
| `304` | `InvalidComparison` | Oracle comparison operator is empty or unsupported. |
| `401` | `InvalidInput` | One or more parameters failed validation. |
| `494` | `InvalidState` | Contract or market is in an unexpected/illegal state. |

> This table lists only the error codes that are reachable from production
> entrypoints. Codes that have been deprecated and pruned are listed in the
> [Deprecated codes](#deprecated-codes) section below.

---

## Error-variant lifecycle policy

The error catalog is a public ABI surface: client SDKs match on the numeric
discriminants. To keep the catalog bounded and meaningful, every variant
follows the lifecycle below.

### Adding a new code

1. A new variant may only be added when it is reachable from at least one
   production entrypoint (or a validator invoked by one) and when no existing
   variant already expresses the same condition.
2. New variants take the next unused discriminant within their range
   (`1xx` market, `2xx` oracle, `3xx` validation, `4xx` input/state). Gaps left
   by pruned codes are **never** filled.
3. The variant must be added to the Full Error Table above with its code, name,
   description, and the entrypoints/conditions that produce it.
4. The variant must have a corresponding `RecoveryStrategy` mapping so SDKs can
   react deterministically.

### Deprecating a code

1. A variant is a deprecation candidate when it is unreachable from every
   production entrypoint (e.g. the code path was removed or refactored).
2. Deprecated variants are marked `#[deprecated]` in `err.rs` and moved to the
   [Deprecated codes](#deprecated-codes) table, retaining their original
   discriminant and description.
3. Deprecated variants remain in the contract for at least one release cycle so
   that existing SDKs continue to compile and decode them.

### Removing a code

1. After the deprecation window, an unreachable variant is removed from
   `err.rs` and from the Deprecated codes table.
2. Removal is recorded in the changelog. The discriminant is retired: it is
   never renumbered and never reused for a different condition.

### Stability guarantee

- Existing discriminants are **never renumbered**.
- Retired discriminants are **never reused** for a different error condition.
- Removing a variant is a breaking change for SDKs only if they matched on it
  explicitly; the numeric value will simply no longer be produced.

---

## Deprecated codes

The following codes have been deprecated and are scheduled for removal. They
are retained here (and in `err.rs`) for the deprecation window only.

| Code | Name | Reason |
|---|---|---|
| _(none currently)_ | | |
