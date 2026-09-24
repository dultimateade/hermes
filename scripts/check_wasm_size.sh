#!/bin/bash
set -e

# Strict error handling: exit on any error or undefined variable
set -u

# Default budget: 768 KiB = 768 * 1024 = 786432 bytes
BUDGET=${WASM_SIZE_BUDGET:-786432}

# Get the target contract name from Cargo.toml in the current directory
# If no Cargo.toml is found, assume this is a workspace build
if [ -f "Cargo.toml" ]; then
  CONTRACT_NAME=$(grep -m1 '^name = ' Cargo.toml | sed 's/name = "\(.*\)"/\1/')
else
  # For workspace builds, we'll validate after finding the wasm file
  CONTRACT_NAME=""
fi

echo "Building contract in release mode..."
cargo build --release --target wasm32v1-none 2>&1

# Find all wasm files
WASM_FILES=$(find target/wasm32v1-none/release -name "*.wasm" -type f 2>/dev/null || echo "")

if [ -z "$WASM_FILES" ]; then
  echo "Error: No WASM files found in target/wasm32v1-none/release"
  echo "This typically means the build failed or the cargo workspace configuration is incorrect."
  exit 1
fi

# If we know the contract name, find the matching WASM file
if [ -n "$CONTRACT_NAME" ]; then
  BASE_WASM_FILE=""
  
  # Look for exact match: contract_name.wasm
  for wasm_file in $WASM_FILES; do
    filename=$(basename "$wasm_file")
    
    # Check if filename matches contract name (handle underscores/hyphens conversions)
    if [ "$filename" = "${CONTRACT_NAME}.wasm" ] || [ "$filename" = "${CONTRACT_NAME//-/_}.wasm" ] || [ "$filename" = "${CONTRACT_NAME//_/-}.wasm" ]; then
      BASE_WASM_FILE="$wasm_file"
      break
    fi
  done
  
  if [ -z "$BASE_WASM_FILE" ]; then
    echo "Error: Expected WASM file '${CONTRACT_NAME}.wasm' not found in target/wasm32v1-none/release"
    echo "Found these WASM files instead:"
    echo "$WASM_FILES" | sed 's/^/  /'
    echo ""
    echo "This suggests the crate name in Cargo.toml may have been changed without updating the build."
    echo "Expected crate name: $CONTRACT_NAME"
    exit 1
  fi
else
  # For workspace builds with multiple contracts, just pick the first one
  # and warn the user that they should specify which contract to check
  BASE_WASM_FILE=$(echo "$WASM_FILES" | head -n 1)
  
  # Check if multiple files were found
  WASM_COUNT=$(echo "$WASM_FILES" | wc -l)
  if [ "$WASM_COUNT" -gt 1 ]; then
    echo "Warning: Multiple WASM files found. Checking the first one: $BASE_WASM_FILE"
    echo "For stricter validation, run this script from within a specific contract directory."
    echo "Found files:"
    echo "$WASM_FILES" | sed 's/^/  /'
  fi
fi

echo "Base WASM: $BASE_WASM_FILE"

# Verify the file actually exists (defense in depth)
if [ ! -f "$BASE_WASM_FILE" ]; then
  echo "Error: WASM file does not exist: $BASE_WASM_FILE"
  exit 1
fi

# Optimize with stellar contract optimize if available
if command -v stellar &> /dev/null; then
  echo "Optimizing WASM with stellar contract optimize..."
  stellar contract optimize --wasm "$BASE_WASM_FILE" 2>&1 || echo "Warning: WASM optimization failed, continuing with unoptimized binary"
fi

# Use optimized file if it exists, otherwise fall back to base
if [ -f "${BASE_WASM_FILE%.wasm}.optimized.wasm" ]; then
  WASM_FILE="${BASE_WASM_FILE%.wasm}.optimized.wasm"
  echo "Using optimized WASM: $WASM_FILE"
else
  WASM_FILE="$BASE_WASM_FILE"
  echo "Using base WASM: $WASM_FILE"
fi

# Verify the file exists before attempting to get its size
if [ ! -f "$WASM_FILE" ]; then
  echo "Error: WASM file does not exist: $WASM_FILE"
  exit 1
fi

# Handle both Linux and macOS stat commands
if [[ "$OSTYPE" == "darwin"* ]]; then
  SIZE=$(stat -f%z "$WASM_FILE")
else
  SIZE=$(stat -c%s "$WASM_FILE")
fi

echo "WASM file: $WASM_FILE"
echo "Size: $SIZE bytes"
echo "Budget: $BUDGET bytes"

if [ "$SIZE" -gt "$BUDGET" ]; then
  echo "Error: WASM size ($SIZE bytes) exceeds budget ($BUDGET bytes)!"
  BASE_SIZE=$(stat -f%z "$BASE_WASM_FILE" 2>/dev/null || stat -c%s "$BASE_WASM_FILE" 2>/dev/null)
  echo "Note: base WASM size was $BASE_SIZE bytes"
  exit 1
fi

echo "WASM size is within budget."
