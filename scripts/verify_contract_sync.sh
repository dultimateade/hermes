#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target/wasm32v1-none/release"
CONTRACT="${CONTRACT:-}"
STAGING_CONTRACT_ID="${STAGING_CONTRACT_ID:-}"
PRODUCTION_CONTRACT_ID="${PRODUCTION_CONTRACT_ID:-}"
STAGING_NETWORK="${STAGING_NETWORK:-testnet}"
PRODUCTION_NETWORK="${PRODUCTION_NETWORK:-mainnet}"

usage() {
  cat <<'EOF'
Usage: CONTRACT=<package> STAGING_CONTRACT_ID=<id> PRODUCTION_CONTRACT_ID=<id> ./scripts/verify_contract_sync.sh

Environment:
  CONTRACT               Cargo package to build, for example markets
  STAGING_CONTRACT_ID    Deployed staging contract ID
  PRODUCTION_CONTRACT_ID Deployed production contract ID
  STAGING_NETWORK        Stellar CLI network for staging (default: testnet)
  PRODUCTION_NETWORK     Stellar CLI network for production (default: mainnet)
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ -z "$CONTRACT" || -z "$STAGING_CONTRACT_ID" || -z "$PRODUCTION_CONTRACT_ID" ]]; then
  usage >&2
  exit 2
fi

command -v cargo >/dev/null 2>&1 || { echo "Error: cargo is required." >&2; exit 1; }
command -v stellar >/dev/null 2>&1 || { echo "Error: stellar CLI is required." >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "Error: sha256sum is required." >&2; exit 1; }

echo "Building ${CONTRACT}..."
(cd "$ROOT_DIR" && cargo build --release --target wasm32v1-none -p "$CONTRACT")

WASM_NAME="${CONTRACT//-/_}.wasm"
WASM_FILE="${TARGET_DIR}/${WASM_NAME}"
if [[ ! -f "$WASM_FILE" ]]; then
  echo "Error: expected WASM artifact not found: ${WASM_FILE}" >&2
  exit 1
fi

EXPECTED_HASH="$(sha256sum "$WASM_FILE" | awk '{print $1}')"
VERIFY_DIR="$(mktemp -d)"
trap 'rm -rf "$VERIFY_DIR"' EXIT

fetch_and_verify() {
  local environment="$1"
  local contract_id="$2"
  local network="$3"
  local fetched_wasm="${VERIFY_DIR}/${environment}.wasm"
  local actual_hash

  echo "Fetching ${environment} contract ${contract_id} from ${network}..."
  stellar contract fetch \
    --id "$contract_id" \
    --network "$network" \
    --output "$fetched_wasm"

  if [[ ! -s "$fetched_wasm" ]]; then
    echo "Error: ${environment} contract fetch produced no WASM: ${fetched_wasm}" >&2
    exit 1
  fi

  actual_hash="$(sha256sum "$fetched_wasm" | awk '{print $1}')"
  if [[ "$actual_hash" != "$EXPECTED_HASH" ]]; then
    printf 'ERROR: %s contract drift detected (expected %s, got %s)\n' \
      "$environment" "$EXPECTED_HASH" "$actual_hash" >&2
    exit 1
  fi

  printf '%s contract matches %s (%s)\n' "$environment" "$CONTRACT" "$actual_hash"
}

printf 'Expected %s WASM hash: %s\n' "$CONTRACT" "$EXPECTED_HASH"
fetch_and_verify staging "$STAGING_CONTRACT_ID" "$STAGING_NETWORK"
fetch_and_verify production "$PRODUCTION_CONTRACT_ID" "$PRODUCTION_NETWORK"
echo "Contract sync verification passed."