# MARKET_PAYOUT_CURVE_TTL_LEDGERS Design

## Overview

`MARKET_PAYOUT_CURVE_TTL_LEDGERS` is a constant defined in `contracts/markets/src/lib.rs` that controls the time-to-live (TTL) applied to the payout curve configuration stored by the markets contract.

```rust
/// TTL (in ledgers) applied to the market payout curve configuration.
/// Set to ~1 year (6_307_200 ledgers at ~5s per ledger).
pub const MARKET_PAYOUT_CURVE_TTL_LEDGERS: u32 = 6_307_200;
```

At approximately 5 seconds per ledger, `6_307_200` ledgers corresponds to roughly one year of wall-clock time.

## Rationale

The payout curve configuration defines how a market's payout is computed across its price range. It must remain readable for the entire lifetime of a market, including the claim window during which users redeem positions after resolution.

Long-running markets can remain open for many months, and the claim window extends beyond resolution. If the curve configuration were allowed to expire before claims were settled, payout computation would fail or fall back to an incorrect default. Setting the TTL to ~1 year ensures the curve configuration outlives the claim window for long-running markets, so payouts can always be computed from the intended configuration.

## Initialization-Time Validation

Soroban storage enforces a network-level maximum TTL (`env.storage().max_ttl()`). If a requested TTL exceeds this maximum, the value is silently clamped to the maximum. On a network whose `max_ttl` is less than one year, `MARKET_PAYOUT_CURVE_TTL_LEDGERS` would be clamped without any warning, potentially allowing the curve configuration to expire earlier than intended.

To prevent this silent degradation, the contract validates the constant against the network maximum during initialization:

- On initialization, the contract compares `MARKET_PAYOUT_CURVE_TTL_LEDGERS` against `env.storage().max_ttl()`.
- If the network's `max_ttl` is less than `MARKET_PAYOUT_CURVE_TTL_LEDGERS`, initialization fails and returns `InvalidConfig`.
- This surfaces the misconfiguration immediately at deploy/init time rather than allowing a silent clamp that could later cause curve configuration expiry.

## Behavior on Networks with Lower max_ttl

| Network `max_ttl` | Behavior |
| --- | --- |
| `>= MARKET_PAYOUT_CURVE_TTL_LEDGERS` | Initialization succeeds; curve configuration is stored with the full ~1 year TTL. |
| `< MARKET_PAYOUT_CURVE_TTL_LEDGERS` | Initialization fails with `InvalidConfig`; the contract is not usable on that network until the constant is adjusted or the network configuration is raised. |

Because the check happens at initialization, operators learn about an insufficient `max_ttl` before any market is created, avoiding the scenario where a market's curve configuration silently expires before its claim window closes.

## References

- `contracts/markets/src/lib.rs` — `MARKET_PAYOUT_CURVE_TTL_LEDGERS` definition and initialization validation.