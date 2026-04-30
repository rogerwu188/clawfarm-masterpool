#!/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
LEDGER_DIR=${LEDGER_DIR:-/tmp/clawfarm-phase1-ledger}
RPC_URL=${ANCHOR_PROVIDER_URL:-http://127.0.0.1:8899}
ANCHOR_WALLET_PATH=${ANCHOR_WALLET:-"$HOME/.config/solana/id.json"}
VALIDATOR_LOG=${VALIDATOR_LOG:-/tmp/clawfarm-phase1-validator.log}

cleanup() {
  if [ "${VALIDATOR_PID:-}" ]; then
    kill "$VALIDATOR_PID" >/dev/null 2>&1 || true
    wait "$VALIDATOR_PID" 2>/dev/null || true
  fi
}

trap cleanup EXIT INT TERM

cd "$ROOT_DIR"

anchor build

rm -rf "$LEDGER_DIR"
solana-test-validator --reset --ledger "$LEDGER_DIR" --quiet >"$VALIDATOR_LOG" 2>&1 &
VALIDATOR_PID=$!

attempt=0
until solana --url "$RPC_URL" slot >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 30 ]; then
    echo "local validator did not become ready; see $VALIDATOR_LOG" >&2
    exit 1
  fi
  sleep 1
done

solana program deploy \
  target/deploy/clawfarm_masterpool.so \
  --program-id target/deploy/clawfarm_masterpool-keypair.json \
  --upgrade-authority "$ANCHOR_WALLET_PATH" \
  --url "$RPC_URL"

solana program deploy \
  target/deploy/clawfarm_attestation.so \
  --program-id target/deploy/clawfarm_attestation-keypair.json \
  --upgrade-authority "$ANCHOR_WALLET_PATH" \
  --url "$RPC_URL"

ANCHOR_PROVIDER_URL="$RPC_URL" \
ANCHOR_WALLET="$ANCHOR_WALLET_PATH" \
  npx ts-mocha -p ./tsconfig.json -t 1000000 tests/phase1-integration.ts "$@"
