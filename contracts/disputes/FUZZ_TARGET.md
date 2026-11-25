# Disputes Fuzz Target

## Overview

This document describes the `cargo-fuzz` target for the disputes subsystem
of the Predictify Hybrid contract.  The target lives at
`contracts/disputes/fuzz/targets/main.rs` and exercises
`predictify_hybrid::disputes::DisputeManager` via a structured byte corpus.

No new public API was introduced.  The fuzz target and edge-case test suite
are **test-only** additions that do not affect the deployed contract.

---

## Package layout

```
contracts/disputes/
├── Cargo.toml                        # [[test]] entries for err_stab and dispute_edge_cases
├── src/lib.rs                        # placeholder (no_std stub)
├── fuzz/
│   ├── Cargo.toml                    # disputes-fuzz crate; [[bin]] main
│   └── targets/
│       └── main.rs                   # cargo-fuzz entry point
└── tests/
    ├── err_stab.rs                   # error-code stability assertions (pre-existing)
    └── dispute_edge_cases.rs         # focused deterministic edge-case tests (new)
```

---

## Running the fuzzer

Requires the nightly toolchain and `cargo-fuzz`:

```bash
cargo install cargo-fuzz          # one-time setup
rustup install nightly

# run with an in-process fuzzer (libFuzzer)
cargo +nightly fuzz run \
  --fuzz-dir contracts/disputes/fuzz \
  main
```

Crashes are stored in `contracts/disputes/fuzz/artifacts/main/`.

---

## Seeding the corpus

`cargo fuzz` reads and writes its working corpus at
`contracts/disputes/fuzz/corpus/main/`. The directory is created on first run
and is not checked in, so a fresh clone starts from an empty — but valid —
corpus. Every file in the directory is replayed at start-up, and newly
discovered inputs are appended there, which makes it the path to extend when you
want the fuzzer to start from a *meaningful* set of inputs rather than from
random mutations alone.

To seed it, drop one or more raw byte files into a separate seed directory
(for example `contracts/disputes/fuzz/seeds/main/`). Each file is a raw byte
sequence that the action decoder walks one action at a time — the first byte of
each step selects the action (see [Fuzz actions](#fuzz-actions)) and the
remaining bytes are that action's payload. Longer files therefore drive longer
action sequences in a single iteration.

Seed files that exercise the commit-reveal path must carry a preimage of at
least `MIN_PREIMAGE_LEN` bytes (see [Commit-reveal preimage
length](#commit-reveal-preimage-length)). The historical 11-byte prefix is no
longer accepted by the harness: it is below the enforced minimum and would be
rejected before reaching the reveal logic, so it no longer reflects a valid
preimage.

Then either point `cargo fuzz` at that directory with libFuzzer's
`-seed_inputs` flag:

```bash
# the positional argument is the working corpus; -seed_inputs copies the
# curated files into it before fuzzing begins
cargo +nightly fuzz run \
  --fuzz-dir contracts/disputes/fuzz \
  main \
  contracts/disputes/fuzz/corpus/main \
  -- -seed_inputs=contracts/disputes/fuzz/seeds/main
```

or copy the seed files directly into `contracts/disputes/fuzz/corpus/main/`
before the first run. Both approaches leave the fuzzer free to keep growing the
working corpus afterwards.

---

## Commit-reveal preimage length

The commit-reveal scheme binds a commitment to a preimage that is only revealed
during the apply window. A short preimage (the harness previously used an
11-byte prefix) makes the commitment vulnerable to an offline preimage search
before the apply window opens: an attacker can enumerate the small preimage
space and recover the secret ahead of time.

To make that search infeasible, the commit-reveal logic enforces a minimum
preimage length of `MIN_PREIMAGE_LEN` bytes on both the commit and reveal paths.
Commitments or reveals whose preimage is shorter than this minimum are rejected
with `DisputeError` rather than being accepted. The fuzz harness mirrors this
contract-level rule: its corpus prefix is sized to `MIN_PREIMAGE_LEN` so that
seeded inputs represent a valid, sufficiently long preimage instead of the old
11-byte short prefix.

---

## Running the focused edge-case tests

These use the standard test harness and do not require nightly:

```bash
cargo test -p disputes --test dispute_edge_cases
```

---

## Fuzz actions

Each byte slice drives a loop of up to six distinct actions:

| Action (byte mod 6) | Description |
|---------------------|-------------|
| 0 — OpenDispute | Calls `DisputeManager::process_dispute` with a fuzz-derived stake |
| 1 — VoteSimple | Calls `vote_on_dispute(user, market, outcome_str, stake)` |
| 2 — VoteExtended | Calls `vote_on_dispute(user, market, dispute_id, bool, stake, reason)` |
| 3 — ResolveDispute | Calls `DisputeManager::resolve_dispute` |
| 4 — AdvanceLedger | Time-travels the test ledger by up to 10 × `DISPUTE_PERIOD_SECS` |
| 5 — SetStakeCap | Injects a per-user stake cap directly into contract storage |

---

## Boundary conditions covered

- Stake amounts: 0, −1, `MIN_DISPUTE_STAKE − 1`, `MIN_DISPUTE_STAKE`, `i128::MAX`
- Market timing: active, ended-but-in-window, past dispute window
- Duplicate disputes by the same user → `AlreadyDisputed`
- Per-user stake cap enforcement → `DisputeStakeCapExceeded`
- Vote by the dispute opener → `DisputerCannotVote`
- Double-voting by the same voter → `DisputeAlreadyVoted`
- Commit-reveal preimage shorter than `MIN_PREIMAGE_LEN` → `DisputeError`
- Resolution before / after votes are cast
- Arbitrary byte sequences must not panic

---

## Expected error set

The fuzzer accepts only the following errors from dispute entry points.
Any other panic is treated as a crash.

| Variant | Code |
|---------|------|
| `AlreadyDisputed` | 404 |
| `DisputeVoteExpired` | 405 |
| `DisputeVoteDenied` | 406 |
| `DisputeAlreadyVoted` | 407 |
| `DisputeCondNotMet` | 408 |
| `DisputeFeeFailed` | 409 |
| `DisputeError` | 410 |
| `DisputerCannotVote` | 438 |
| `DisputeStakeCapExceeded` | 522 |

General infrastructure errors (`Unauthorized`, `MarketNotFound`, `Overflow`,
etc.) are also permitted when the environment is partially wired during corpus
replay.

---

## Cargo.toml changes

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Added `"contracts/disputes/fuzz"` to `[workspace] members` |
| `contracts/disputes/Cargo.toml` | Added `[[test]] dispute_edge_cases` entry |
| `contracts/disputes/fuzz/Cargo.toml` | Added `arbitrary` dependency; added `[[bin]] main` entry |

---

## Bug fix

Two stray closing braces (`}`) in
`contracts/predictify-hybrid/src/disputes.rs` (lines 2907 and 3053) that
caused an "unexpected closing delimiter" parse error have been removed.
These were orphaned method-close braces duplicated inside the `DisputeUtils`
`impl` block.
