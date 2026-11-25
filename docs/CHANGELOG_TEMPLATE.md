# Changelog Entry Template

Use this template when updating `CHANGELOG.md` for your PR. Add your entries to the appropriate subsection under `[Unreleased]`.

> **Required:** Every entry under `[Unreleased]` must end with a reference to a GitHub issue, in the form `(#NNN)` or `closes #NNN`. The referenced issue must exist in this repository. CI (`.github/workflows/changelog-check.yml`) will fail the PR if an entry is missing a reference or references an issue that does not exist.

## Examples

### Feature Addition

```markdown
### Added
- **markets**: New price feed validation ensures oracle prices are within acceptable bounds (#1234)
- **disputes**: Support for multi-signature dispute approval process (closes #1235)
```

### Bug Fix

```markdown
### Fixed
- **betting**: Corrected fee calculation overflow when bet amount exceeds u128 threshold (#1234)
- **markets**: Fixed race condition in concurrent bet placement (closes #1236)
```

### Breaking Change / Deprecation

```markdown
### Changed
- **fees**: Fee calculation now deducts platform fees before user rewards distribution (#1237)
- **oracles**: Simplified oracle price submission interface (closes #1238)

### Deprecated
- **markets**: `legacy_claim_winnings` entry point (use `claim_winnings` instead) (#1239)
```

### Security Fix

```markdown
### Security
- **admin**: Fixed authorization bypass in `transfer_ownership` that allowed non-admin transfers (#1240)
- **disputes**: Prevented double-spending in dispute resolution awards (closes #1241)
```

## Guidelines

1. **Start each bullet with a contract name in bold**: `- **contract_name**: Description`
2. **Be specific about what changed**: Instead of "Fixed bug", write "Fixed fee calculation in `place_bet` that overflowed on large amounts"
3. **Reference the issue (required)**: End every entry with `(#1234)` or `closes #1234`. The referenced issue must exist in the repository; CI (`changelog-check.yml`) rejects entries without a valid issue reference
4. **Group related changes**: If multiple related fixes are in one PR, group them under one subsection
5. **Explain the impact**: Help users understand why this change matters

## When to Add Entries

Add entries whenever you modify:
- Contract code in `contracts/*/src/`
- Core utilities in `crates/*/src/`
- Test infrastructure (when it affects behavior, not all test changes)

Do NOT add entries for:
- Documentation-only changes
- Formatting/style changes
- CI/build configuration changes (unless they affect the build output)

## Release Process

When preparing a release:

1. Review all entries in `[Unreleased]`
2. Create a new section: `## [X.Y.Z] - YYYY-MM-DD`
3. Move all entries from `[Unreleased]` to the new section
4. Add version link at the bottom of the file
5. Commit and tag the release
