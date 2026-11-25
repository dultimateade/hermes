# Changelog Quick Reference

## Where is CHANGELOG.md?

The file is at the repository root: `/CHANGELOG.md`

## When do I need to update it?

**Always** when your PR modifies files in:
- `contracts/*/src/` - any contract code changes
- `crates/*/src/` - any core utility changes

**Do NOT** update it for:
- Documentation changes (unless they affect public APIs)
- Build/CI configuration changes
- Test infrastructure changes (unless they affect behavior)
- Formatting or style-only changes

## How do I update it?

1. Open `CHANGELOG.md`
2. Find the `## [Unreleased]` section (should be near the top)
3. Add your entry under the appropriate subsection:
   - **Added** - new features or contracts
   - **Changed** - changes to existing functionality
   - **Deprecated** - features being phased out
   - **Removed** - removed features
   - **Fixed** - bug fixes
   - **Security** - security-related changes

## Format Rules

- Start each line with `- **contract_name**: Description`
- Reference your PR or issue number: `(#1234)` at the end
- Be specific: "Fixed fee calculation overflow" not "Fixed bug"
- One line per change, but related changes can be grouped

## Examples

### Good ✓

```markdown
### Fixed
- **markets**: Corrected fee calculation in `place_bet` that overflowed on large amounts (#1234)
- **betting**: Fixed race condition in concurrent bet placement

### Added
- **disputes**: New multi-signature approval workflow for dispute resolution (#1235)
```

### Not Good ✗

```markdown
### Fixed
- Fixed stuff
- Bug fix

### Added
- New features
```

## CI Check

A GitHub Actions workflow (`changelog-check.yml`) automatically verifies that:
1. `CHANGELOG.md` was modified if your PR touches contracts or core code
2. The `[Unreleased]` section has meaningful entries
3. The format follows Keep a Changelog

**If the check fails**, you'll get:
- A CI status failure on your PR
- A comment on the PR with instructions
- A link to this documentation

Simply update `CHANGELOG.md` and push again to fix it.

## Release Process

When a release is prepared:
1. A maintainer creates a new section like `## [1.0.0] - 2024-10-21`
2. All entries from `[Unreleased]` are moved to that section
3. Version comparison links are added at the bottom
4. The repository is tagged with the version
5. A GitHub release is created with the CHANGELOG entries

You don't need to do anything for this—just keep the `[Unreleased]` section updated as you work.

## Questions?

- See the full [Keep a Changelog](https://keepachangelog.com/) specification
- Check existing entries in `CHANGELOG.md` for examples
- See `docs/CHANGELOG_TEMPLATE.md` for detailed templates
- See `CONTRIBUTING.md` for general contribution guidelines
