# Contributing to Hermes

Thank you for contributing! This document covers the development workflow,
testing requirements, and operational scripts you need to know about.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, `wasm32v1-none` target)
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-stellar-cli)
- `sha256sum` (standard on Linux; `brew install coreutils` on macOS)

## Running Tests

```bash
cargo test --workspace
```

Auth boundary tests for individual contracts can be run with:

```bash
cargo test --test auth_boundary -- --nocapture
```

## Verifying Contract Sync

`scripts/verify_contract_sync.sh` checks that a locally-built WASM artifact
matches the bytecode currently deployed on staging and production. Run it after
any deployment, or before merging a release branch, to detect contract drift.

```bash
CONTRACT=markets \
STAGING_CONTRACT_ID=<staging-contract-id> \
PRODUCTION_CONTRACT_ID=<production-contract-id> \
./scripts/verify_contract_sync.sh
```

Pass `--help` for the full list of options:

```bash
./scripts/verify_contract_sync.sh --help
```

The script requires `cargo`, `stellar`, and `sha256sum` to be available on
your `PATH`. It builds the contract in release mode, fetches the deployed
bytecode via the Stellar CLI, and compares SHA-256 hashes. A non-zero exit
code means the deployed contract does not match the local build.

## Pull Request Guidelines

- Keep PRs focused on a single concern.
- All CI checks (tests + snapshot goldens) must pass before merge.
- Do not push directly to `main`; open a pull request instead.
- **For contract or core code changes, update CHANGELOG.md** in the `[Unreleased]` section describing your changes
- For contract changes, run `verify_contract_sync.sh` post-deploy and attach
  the output to your PR description.

## Updating the Changelog

If your PR modifies code in `contracts/` or `crates/`, you must update `CHANGELOG.md`:

1. Open `CHANGELOG.md` and locate the `[Unreleased]` section
2. Add your changes under the appropriate subsection:
   - **Added**: New features or contracts
   - **Changed**: Changes to existing functionality
   - **Deprecated**: Features being phased out
   - **Removed**: Removed features
   - **Fixed**: Bug fixes
   - **Security**: Security-related changes
3. Be descriptive and indicate which contract(s) are affected

Example:
```markdown
### Fixed
- **markets**: Corrected fee calculation in `place_bet` (#1234)
- **disputes**: Fixed race condition in dispute resolution

### Added
- **validators**: New validator eligibility check in initialization
```

See [Keep a Changelog](https://keepachangelog.com/) for detailed guidelines on formatting.
