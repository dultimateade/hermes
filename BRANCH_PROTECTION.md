# Branch Protection Rules

This document specifies the branch protection rules that should be configured for the `main` branch to prevent broken code from being merged without validation.

## Configuration

### Pattern
- **Branch name pattern:** `main`

### Required Status Checks
The following CI jobs must pass before a pull request can be merged:

1. **test** - Cargo workspace tests
   - Runs all tests in the workspace
   - Verifies the Rust toolchain is correct
   - Validates snapshot goldens are committed and unchanged

### Additional Recommended Settings

- **Require a pull request before merging:** Yes
- **Require approvals:** At least 1 (optional, based on team policy)
- **Require status checks to pass before merging:** Yes
- **Require branches to be up to date before merging:** Yes (recommended)
- **Include administrators:** No (allows admins to merge urgent fixes)

## Setup Instructions

To configure these rules via the GitHub UI:

1. Navigate to your repository's **Settings** tab
2. Select **Branches** from the left sidebar
3. Click **Add rule** under "Branch protection rules"
4. Enter `main` in the "Branch name pattern" field
5. Enable the following checkboxes:
   - ✓ Require a pull request before merging
   - ✓ Require status checks to pass before merging
   - ✓ Require branches to be up to date before merging (recommended)
6. Under "Require status checks to pass before merging", search for and select:
   - ✓ test
7. Click **Create**

## CI Workflow

The CI workflow (`.github/workflows/ci.yml`) is configured to run on:
- Pull requests to any branch
- Push events to the `main` branch

This ensures that all code changes are validated before merge and that the main branch always maintains a passing build state.
