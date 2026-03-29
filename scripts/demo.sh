#!/usr/bin/env bash
# LeaseFlow Demo — compile → optimize → deploy → run full lease lifecycle on Testnet
set -euo pipefail

NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
WASM_PATH="target/wasm32-unknown-unknown/release/leaseflow_contracts.wasm"
WASM_OPT_PATH="target/wasm32-unknown-unknown/release/leaseflow_contracts.optimized.wasm"

STELLAR="${STELLAR:-./stellar}"

# ── helpers ──────────────────────────────────────────────────────────────────
log()  { echo -e "\033[1;34m[leaseflow]\033[0m $*"; }
ok()   { echo -e "\033[1;32m  ✓\033[0m $*"; }
die()  { echo -e "\033[1;31m  ✗\033[0m $*" >&2; exit 1; }

require_cmd() { command -v "$1" &>/dev/null || die "'$1' not found. Install it and retry."; }

# ── preflight ─────────────────────────────────────────────────────────────────
require_cmd cargo
require_cmd wasm-opt || true   # optional — we skip optimisation if absent
[[ -x "$STELLAR" ]] || require_cmd stellar && STELLAR=stellar

# ── 1. build ──────────────────────────────────────────────────────────────────
log "Building contract (release)…"
cargo build --release --target wasm32-unknown-unknown \
  --package leaseflow_contracts 2>&1 | tail -5
ok "WASM built → $WASM_PATH"

# ── 2. optimise ───────────────────────────────────────────────────────────────
if command -v wasm-opt &>/dev/null; then
  log "Optimising WASM…"
  wasm-opt -Oz --strip-debug "$WASM_PATH" -o "$WASM_OPT_PATH"
  ok "Optimised → $WASM_OPT_PATH"
else
  log "wasm-opt not found — skipping optimisation (install binaryen for smaller output)"
  WASM_OPT_PATH="$WASM_PATH"
fi

# ── 3. fund demo accounts ─────────────────────────────────────────────────────
log "Generating / funding demo accounts on Testnet…"

fund_account() {
  local alias="$1"
  if ! $STELLAR keys address "$alias" &>/dev/null; then
    $STELLAR keys generate "$alias" --network "$NETWORK" --fund
    ok "Created & funded: $alias ($(${STELLAR} keys address "$alias"))"
  else
    # top-up in case balance is low
    curl -sf "https://friendbot.stellar.org?addr=$($STELLAR keys address "$alias")" \
      -o /dev/null && ok "Topped-up: $alias" || true
  fi
}

fund_account "lf-landlord"
fund_account "lf-tenant"

LANDLORD_ADDR=$($STELLAR keys address "lf-landlord")
TENANT_ADDR=$($STELLAR keys address "lf-tenant")

log "Landlord : $LANDLORD_ADDR"
log "Tenant   : $TENANT_ADDR"

# ── 4. deploy ─────────────────────────────────────────────────────────────────
log "Deploying contract to Testnet…"
CONTRACT_ID=$($STELLAR contract deploy \
  --wasm "$WASM_OPT_PATH" \
  --source "lf-landlord" \
  --network "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE")
ok "Deployed → $CONTRACT_ID"

invoke() {
  $STELLAR contract invoke \
    --id "$CONTRACT_ID" \
    --source "lf-landlord" \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- "$@"
}

invoke_as() {
  local signer="$1"; shift
  $STELLAR contract invoke \
    --id "$CONTRACT_ID" \
    --source "$signer" \
    --network "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    -- "$@"
}

# ── 5. create demo lease ──────────────────────────────────────────────────────
log "Creating demo lease…"
# Uses the simple create_lease entry-point (no NFT, no KYC gate in demo mode).
# payment_token is the native XLM asset contract on testnet.
NATIVE_TOKEN="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"

LEASE_ID=$(invoke \
  create_lease \
  --landlord "$LANDLORD_ADDR" \
  --tenant   "$TENANT_ADDR" \
  --_amount  1000 \
  --payment_token "$NATIVE_TOKEN")
ok "Lease created → id: $LEASE_ID"

# ── 6. activate lease ─────────────────────────────────────────────────────────
log "Tenant activating lease…"
invoke_as "lf-tenant" \
  activate_lease \
  --lease_id "$LEASE_ID" \
  --tenant   "$TENANT_ADDR"
ok "Lease activated"

# ── 7. pay rent ───────────────────────────────────────────────────────────────
log "Tenant paying first rent instalment (500 stroops)…"
invoke_as "lf-tenant" \
  pay_rent \
  --lease_id       "$LEASE_ID" \
  --payment_amount 500
ok "Rent paid"

# ── 8. summary ────────────────────────────────────────────────────────────────
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  LeaseFlow Demo — Full Lifecycle Complete"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Network    : Stellar Testnet"
echo "  Contract   : $CONTRACT_ID"
echo "  Landlord   : $LANDLORD_ADDR"
echo "  Tenant     : $TENANT_ADDR"
echo "  Lease ID   : $LEASE_ID"
echo ""
echo "  Inspect on Stellar Expert:"
echo "  https://stellar.expert/explorer/testnet/contract/$CONTRACT_ID"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
