#!/bin/bash
set -euo pipefail

# Benchmark Regression Check Runner for Hermes Contracts
# Usage: ./scripts/check_benchmarks.sh [--contract <name>] [--thresholds <path>] [additional check_benchmarks.py flags]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

# Default parameters
CONTRACT="analytics"
THRESHOLDS="contracts/analytics/benches/thresholds.json"

# Run Python benchmark runner
python3 "${SCRIPT_DIR}/check_benchmarks.py" --contract "${CONTRACT}" --thresholds "${THRESHOLDS}" "$@"
