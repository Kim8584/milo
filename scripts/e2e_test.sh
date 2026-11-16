#!/usr/bin/env bash
set -euo pipefail

echo "=== Milo End-to-End Pipeline Verification ==="

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="${ROOT_DIR}/target/release/milo"

# 1. Build release binary if not present
if [ ! -f "${BIN_PATH}" ]; then
    echo "Building milo in release mode..."
    cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"
fi

# 2. Verify sample circuits off-chain via Arkworks
echo "Testing off-chain verification (multiplier2)..."
"${BIN_PATH}" verify \
    --vk "${ROOT_DIR}/examples/circuits/multiplier2_vk.json" \
    --proof "${ROOT_DIR}/examples/circuits/multiplier2_proof.json" \
    --public "${ROOT_DIR}/examples/circuits/multiplier2_public.json"

echo "Testing off-chain verification (poly4)..."
"${BIN_PATH}" verify \
    --vk "${ROOT_DIR}/examples/circuits/poly4_vk.json" \
    --proof "${ROOT_DIR}/examples/circuits/poly4_proof.json" \
    --public "${ROOT_DIR}/examples/circuits/poly4_public.json"

# 3. Test code generation into a temporary directory
TEMP_OUT="$(mktemp -d)"
trap 'rm -rf "${TEMP_OUT}"' EXIT

echo "Testing Move code generation into ${TEMP_OUT}..."
"${BIN_PATH}" generate \
    --vk "${ROOT_DIR}/examples/circuits/multiplier2_vk.json" \
    --proof "${ROOT_DIR}/examples/circuits/multiplier2_proof.json" \
    --public "${ROOT_DIR}/examples/circuits/multiplier2_public.json" \
    --module-name e2e_multiplier2 \
    --out-dir "${TEMP_OUT}"

if [ ! -f "${TEMP_OUT}/e2e_multiplier2.move" ]; then
    echo "Error: e2e_multiplier2.move was not generated"
    exit 1
fi

if [ ! -f "${TEMP_OUT}/call_args.json" ]; then
    echo "Error: call_args.json was not generated"
    exit 1
fi

echo "=== All E2E checks passed successfully! ==="
