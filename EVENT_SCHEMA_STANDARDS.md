# Event Schema Standards for Subgraph Indexers

## Overview
To ensure robust off-chain indexability (e.g., The Graph), the LeaseFlow Protocol enforces a strict, standardized event emission schema. This prevents expensive RPC state queries and allows for real-time "Leasing Dashboards".

## Security & Privacy
**NO PII (Personally Identifiable Information)** is ever logged to the immutable ledger. Property addresses, tenant names, and private metadata must be kept off-chain (e.g., in IPFS). Only cryptographic hashes (`DataHash`) are stored on-chain to verify integrity.

## Standard Event Structure
Every state mutation emits an event using the following rigid vector format:

### Topics (Indexed for fast filtering)
`[Action: Symbol, LeaseID: u64, ReasonCode: u32]`

*   **Action**: A `Symbol` representing the event type (e.g., `CREATED`, `SLASHED`, `EVICTED`).
*   **LeaseID**: The unique `u64` identifier for the lease.
*   **ReasonCode**: (Optional but required for failures/slashes) Maps directly to the Soroban Error Enum.

### Data Payload (Not indexed)
`[Timestamp: u64, DataHash: BytesN<32>, Amount: i128]`

*   **Timestamp**: The exact ledger timestamp of execution.
*   **DataHash**: `BytesN<32>` containing the IPFS CID hash or state hash.
*   **Amount**: Relevant token amounts shifted (e.g., slash amount, rent paid).

## Specific Event Types

### Lease Created
- **Topics**: `["CREATED", 104]`
- **Data**: `[1712000000, 0xab12..., 5000000000]` *(Start Time, Meta Hash, Deposit)*

### Deposit Slashed
- **Topics**: `["SLASHED", 104, 51]` *(51 = DamageExceedsDeposit error code map)*
- **Data**: `[1712050000, 0xef45..., 1500000000]` *(Slash Time, Oracle Report Hash, Slash Amount)*

### Eviction
- **Topics**: `["EVICTED", 104, 12]` *(12 = Arrears threshold met)*
- **Data**: `[1712090000, 0x0000..., 0]`

UI teams can execute simple GraphQL subscriptions against the Subgraph using these predictable filters.