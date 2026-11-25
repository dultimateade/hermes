# Monitor Fuzz Target

## Overview

This document describes the `cargo-fuzz` target for the Monitor contract.
The target lives at `contracts/monitor/fuzz/targets/cap_sequence.rs` and
exercises `MonitorContract` via a structured byte corpus.

No new public API was introduced.  The fuzz target is a **test-only** addition
that does not affect the deployed contract.

---

## Package layout

```
contracts/monitor/
├── Cargo.toml
├── src/
│   ├── lib.rs         # contract entry points
│   ├── limits.rs      # CapType, cap constants, enforcement helpers
│   ├── errors.rs      # MonitorError enum
│   ├── events.rs      # event emitters
│   └── storage.rs     # DataKey enum
├── fuzz/
│   ├── Cargo.toml     # monitor-fuzz crate; [[bin]] cap_sequence
│   └── targets/
│       └── cap_sequence.rs   # cargo-fuzz entry point
└── tests/
    ├── limits.rs
    ├── auth_boundary.rs
    └── err_stab.rs
```

---

## Invariants under test

After every `record_*`, `remove_*`, and `set_caps` call in the fuzz sequence
the target asserts:

| Invariant | Condition checked |
|-----------|-------------------|
| **No counter below zero** | `count < WRAP_SENTINEL` (10 001), which catches a `u32` wrap-around caused by a broken underflow guard |
| **No counter above its cap** | `count <= cap` for each resource type |

Both invariants are checked for every user in the pool after every step.

---

## Running the fuzzer

Requires the nightly toolchain and `cargo-fuzz`:

```bash
cargo install cargo-fuzz          # one-time setup
rustup install nightly

cargo +nightly fuzz run \
  --fuzz-dir contracts/monitor/fuzz \
  cap_sequence
```

Crashes are stored in `contracts/monitor/fuzz/artifacts/cap_sequence/`.

---

## Seeding the corpus

The working corpus lives at `contracts/monitor/fuzz/corpus/cap_sequence/`
and is not checked in.  To seed it, place raw byte files in a seed directory
and pass them with libFuzzer's `-seed_inputs` flag:

```bash
cargo +nightly fuzz run \
  --fuzz-dir contracts/monitor/fuzz \
  cap_sequence \
  contracts/monitor/fuzz/corpus/cap_sequence \
  -- -seed_inputs=contracts/monitor/fuzz/seeds/cap_sequence
```

---

## Fuzz actions

Each byte slice drives a loop of up to nine distinct actions.  Every action
consumes exactly **2 bytes**: an opcode byte and a parameter byte.

| Action (opcode byte mod 9) | Description | Parameter byte meaning |
|----------------------------|-------------|------------------------|
| 0 — `record_bet`           | `client.try_record_bet(user)` | user index (mod 3) |
| 1 — `remove_bet`           | `client.try_remove_bet(user)` | user index |
| 2 — `record_position`      | `client.try_record_position(user)` | user index |
| 3 — `remove_position`      | `client.try_remove_position(user)` | user index |
| 4 — `record_subscription`  | `client.try_record_subscription(user)` | user index |
| 5 — `remove_subscription`  | `client.try_remove_subscription(user)` | user index |
| 6 — `set_caps(Bets, cap)`  | `client.try_set_caps(admin, Bets, cap)` | cap value (1–255) |
| 7 — `set_caps(Positions, cap)` | `client.try_set_caps(admin, Positions, cap)` | cap value |
| 8 — `set_caps(Subscriptions, cap)` | `client.try_set_caps(admin, Subscriptions, cap)` | cap value |

---

## Allowed errors

The target accepts only the following errors from contract entry points.
Any other error (or any panic) is treated as a crash.

| Entry point | Allowed errors |
|-------------|----------------|
| `record_bet` | `BetCapExceeded` |
| `remove_bet` | `Underflow` |
| `record_position` | `PositionCapExceeded` |
| `remove_position` | `Underflow` |
| `record_subscription` | `SubscriptionCapExceeded` |
| `remove_subscription` | `Underflow` |
| `set_caps` | `InvalidInput` |

---

## Cargo.toml changes

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Added `"contracts/monitor/fuzz"` to `[workspace] members` |
| `contracts/monitor/fuzz/Cargo.toml` | New file — `monitor-fuzz` crate with `[[bin]] cap_sequence` |
