# Webhook Integration Guide

This guide shows how to parse `RentPaymentExecuted` and `DepositSlashed` Soroban events in JavaScript and Python so that Web2 backends can react to on-chain lease activity without XDR decoding expertise.

## Overview

Both events carry a `lessor_reference_id` field — an opaque string set by the landlord at lease creation (or later via `set_lessor_reference_id`). This lets your backend map a Stellar public key to an internal user ID without maintaining a separate lookup table.

```
lessor_reference_id = "usr_8f3a2c"   // your internal user/merchant ID
```

## Event Schemas

### RentPaymentExecuted

| Field | Type | Description |
|---|---|---|
| `lease_id` | `u64` | On-chain lease identifier |
| `payer` | `Address` | Tenant's Stellar address |
| `amount` | `i128` | Amount paid (in token's smallest unit) |
| `timestamp` | `u64` | Ledger timestamp of payment |
| `lessor_reference_id` | `Option<String>` | Landlord's Web2 reference ID |

### DepositSlashed

| Field | Type | Description |
|---|---|---|
| `lease_id` | `u64` | On-chain lease identifier |
| `oracle_pubkey` | `BytesN<32>` | Oracle that authorised the slash |
| `damage_code` | `u32` | Damage severity (0=normal wear, 5=catastrophic) |
| `deducted_amount` | `i128` | Amount deducted from deposit |
| `tenant_refund` | `i128` | Amount returned to tenant |
| `landlord_payout` | `i128` | Amount paid to landlord |
| `lessor_reference_id` | `Option<String>` | Landlord's Web2 reference ID |

---

## JavaScript (Node.js)

```js
import { SorobanRpc, xdr } from "@stellar/stellar-sdk";

const server = new SorobanRpc.Server("https://soroban-testnet.stellar.org");
const CONTRACT_ID = "CAEGD57WVTVQSYWYB23AISBW334QO7WNA5XQ56S45GH6BP3D2AVHKUG4";

async function pollEvents(startLedger) {
  const { events } = await server.getEvents({
    startLedger,
    filters: [
      {
        type: "contract",
        contractIds: [CONTRACT_ID],
        topics: [["RentPaymentExecuted"], ["DepositSlashed"]],
      },
    ],
  });

  for (const event of events) {
    const topic = event.topic[0].value(); // event name symbol
    const data = event.value.value();     // Vec of ScVal fields

    if (topic === "RentPaymentExecuted") {
      const payload = {
        lease_id:            data[0].u64(),
        payer:               data[1].address().toString(),
        amount:              data[2].i128().toString(),
        timestamp:           data[3].u64(),
        lessor_reference_id: data[4].switch().name === "scvVoid"
                               ? null
                               : data[4].str().toString(),
      };
      await dispatchWebhook("rent.paid", payload);
    }

    if (topic === "DepositSlashed") {
      const payload = {
        lease_id:            data[0].u64(),
        damage_code:         data[2].u32(),
        deducted_amount:     data[3].i128().toString(),
        tenant_refund:       data[4].i128().toString(),
        landlord_payout:     data[5].i128().toString(),
        lessor_reference_id: data[6].switch().name === "scvVoid"
                               ? null
                               : data[6].str().toString(),
      };
      await dispatchWebhook("deposit.slashed", payload);
    }
  }
}

async function dispatchWebhook(event, payload) {
  // Forward to your backend, Zapier, AWS Lambda, etc.
  await fetch(process.env.WEBHOOK_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ event, payload }),
  });
}
```

---

## Python

```python
import httpx
from stellar_sdk import SorobanServer
from stellar_sdk.soroban_rpc import GetEventsRequest, EventFilter

SERVER_URL = "https://soroban-testnet.stellar.org"
CONTRACT_ID = "CAEGD57WVTVQSYWYB23AISBW334QO7WNA5XQ56S45GH6BP3D2AVHKUG4"
WEBHOOK_URL = "https://your-backend.example.com/webhooks/leaseflow"

def poll_events(start_ledger: int):
    server = SorobanServer(SERVER_URL)
    resp = server.get_events(
        start_ledger=start_ledger,
        filters=[
            EventFilter(
                event_type="contract",
                contract_ids=[CONTRACT_ID],
                topics=[["RentPaymentExecuted"], ["DepositSlashed"]],
            )
        ],
    )

    for event in resp.events:
        topic = event.topic[0]          # symbol ScVal
        fields = event.value.values()   # list of ScVal

        if topic == "RentPaymentExecuted":
            payload = {
                "lease_id":            fields[0].u64,
                "payer":               str(fields[1].address),
                "amount":              str(fields[2].i128),
                "timestamp":           fields[3].u64,
                "lessor_reference_id": fields[4].str if fields[4].type != "void" else None,
            }
            dispatch_webhook("rent.paid", payload)

        elif topic == "DepositSlashed":
            payload = {
                "lease_id":            fields[0].u64,
                "damage_code":         fields[2].u32,
                "deducted_amount":     str(fields[3].i128),
                "tenant_refund":       str(fields[4].i128),
                "landlord_payout":     str(fields[5].i128),
                "lessor_reference_id": fields[6].str if fields[6].type != "void" else None,
            }
            dispatch_webhook("deposit.slashed", payload)

def dispatch_webhook(event: str, payload: dict):
    httpx.post(WEBHOOK_URL, json={"event": event, "payload": payload})
```

---

## Security Notes

- **SQL injection**: `lessor_reference_id` is validated on-chain to contain only ASCII printable characters (0x20–0x7E) before storage. Your backend should still treat it as untrusted input and use parameterised queries.
- **Replay attacks**: Always verify `lease_id` + `timestamp` against your database before acting on a webhook to prevent duplicate processing.
- **Authenticity**: Events are emitted by the contract itself; verify the `contractId` in the event matches the deployed contract address before processing.
