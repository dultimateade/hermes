#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target/wasm32v1-none/release"
CONTRACT="${CONTRACT:-}"
SOURCE_ACCOUNT="${SOURCE_ACCOUNT:-}"
NETWORK="${NETWORK:-testnet}"

usage() {
  cat <<'EOF'
Usage: CONTRACT=<package> SOURCE_ACCOUNT=<identity> ./scripts/deploy_testnet.sh

Environment:
  CONTRACT         Cargo package to build and deploy, for example markets
  SOURCE_ACCOUNT   Stellar CLI identity that pays for deployment
  NETWORK          Stellar CLI network (default: testnet)
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ -z "$CONTRACT" || -z "$SOURCE_ACCOUNT" ]]; then
  usage >&2
  exit 2
fi

command -v cargo >/dev/null 2>&1 || { echo "Error: cargo is required." >&2; exit 1; }
command -v stellar >/dev/null 2>&1 || { echo "Error: stellar CLI is required." >&2; exit 1; }

echo "Building ${CONTRACT} for ${NETWORK}..."
(cd "$ROOT_DIR" && cargo build --release --target wasm32v1-none -p "$CONTRACT")

WASM_NAME="${CONTRACT//-/_}.wasm"
WASM_FILE="${TARGET_DIR}/${WASM_NAME}"
if [[ ! -f "$WASM_FILE" ]]; then
  echo "Error: expected WASM artifact not found: ${WASM_FILE}" >&2
  exit 1
fi

echo "Deploying ${WASM_FILE} with identity ${SOURCE_ACCOUNT}..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_FILE" \
  --source-account "$SOURCE_ACCOUNT" \
  --network "$NETWORK")

printf 'Deployed %s to %s: %s\n' "$CONTRACT" "$NETWORK" "$CONTRACT_ID"