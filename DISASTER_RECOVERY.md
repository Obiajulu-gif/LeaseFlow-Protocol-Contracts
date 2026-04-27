# Disaster Recovery Runbook

## Overview
This runbook details the exact procedures for worst-case scenarios on the LeaseFlow Protocol mainnet. It outlines the multi-sig coordination process to authorize a Wasm upgrade or trigger an emergency protocol freeze, and includes exact CLI commands for state migrations.

## Emergency Freeze Execution
In the event of an active exploit, the Security Council must immediately pause the protocol to stop all state mutations.

1. **Multi-sig Coordination:** Minimum 3-of-5 Security Council members must sign the freeze payload.
2. **Execute Freeze Command:**
```bash
soroban contract invoke \
  --network mainnet \
  --id <LEASEFLOW_CONTRACT_ID> \
  --source-account <MULTI_SIG_ADMIN> \
  -- \
  emergency_freeze
```

## Wasm Upgrade Procedure (V1 to V2)
To patch a vulnerability, the protocol logic must be upgraded while preserving state.

1. **Install New Wasm on-chain:**
```bash
soroban contract install \
  --network mainnet \
  --wasm target/wasm32-unknown-unknown/release/leaseflow_v2.wasm \
  --source-account <DEPLOYER_ACCOUNT>
```
*(Save the returned WASM hash)*

2. **Upgrade Contract via Multi-sig:**
```bash
soroban contract invoke \
  --network mainnet \
  --id <LEASEFLOW_CONTRACT_ID> \
  --source-account <MULTI_SIG_ADMIN> \
  -- \
  upgrade \
  --new_wasm_hash <NEW_WASM_HASH>
```

## Emergency State Migration (Trapped User State)
If user funds are trapped due to a state collision, a specific migration script must execute a one-time ledger state dump and re-initialize into V2.

**Dump Current Ledger State:**
```bash
soroban contract read \
  --network mainnet \
  --id <LEASEFLOW_CONTRACT_ID> \
  --output json > trapped_state_dump.json
```

**Migrate State:** Parse `trapped_state_dump.json` off-chain, compute correct balances, and invoke the V2 `emergency_migrate_state` batch function with the corrected Merkle root.