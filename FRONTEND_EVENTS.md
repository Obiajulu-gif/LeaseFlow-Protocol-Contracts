# Frontend Event Integration

This document describes the events that the LeaseFlow Protocol Contracts emit to notify the frontend when assets become available or change state.

## New Events

### 1. LeaseStarted
Emitted when a lease is activated and the asset becomes available to the renter.

**Event Structure:**
```rust
pub struct LeaseStarted {
    pub id: u64,        // Timestamp-based unique ID
    pub renter: Address, // Address of the renter/tenant
    pub rate: i128,     // Per-second rent rate
}
```

**When emitted:** When `activate_lease` is called successfully.

**Frontend use case:** Show that an asset is now available for use by the renter.

### 2. LeaseEnded
Emitted when a lease terminates, providing payment summary information.

**Event Structure:**
```rust
pub struct LeaseEnded {
    pub id: u64,         // Lease ID
    pub duration: u64,   // Total lease duration in seconds
    pub total_paid: i128, // Total amount paid during lease
}
```

**When emitted:** When `terminate_lease` is called successfully.

**Frontend use case:** Update UI to show lease completion and payment summary.

### 3. AssetReclaimed
Emitted when an asset is reclaimed by the landlord or system.

**Event Structure:**
```rust
pub struct AssetReclaimed {
    pub id: u64,        // Lease ID
    pub reason: String, // Reason for reclamation
}
```

**When emitted:** When `reclaim_asset` is called successfully.

**Frontend use case:** Notify that an asset is no longer available and show the reason.

## Integration Guide

### Listening to Events

Frontend applications should listen to these events using the Stellar Soroban SDK event listeners:

```javascript
// Example: Listen to LeaseStarted events
contract.events().on('LeaseStarted', (event) => {
    const { id, renter, rate } = event.data;
    // Update UI to show asset is available
    updateAssetAvailability(id, renter, rate);
});

// Example: Listen to LeaseEnded events  
contract.events().on('LeaseEnded', (event) => {
    const { id, duration, total_paid } = event.data;
    // Update UI to show lease completion
    showLeaseSummary(id, duration, total_paid);
});

// Example: Listen to AssetReclaimed events
contract.events().on('AssetReclaimed', (event) => {
    const { id, reason } = event.data;
    // Update UI to show asset is no longer available
    markAssetUnavailable(id, reason);
});
```

### Event Filtering

Events can be filtered by specific lease IDs or addresses:

```javascript
// Listen to events for a specific lease
contract.events()
    .filter({ lease_id: specificLeaseId })
    .on('LeaseStarted', handleLeaseStarted);

// Listen to events for a specific renter
contract.events()
    .filter({ renter: userAddress })
    .on('LeaseStarted', handleUserLeaseStarted);
```

## Testing

The contract includes comprehensive tests for all events. Run tests with:

```bash
make test
# or
stellar contract test
```

## Migration Notes

These events are additive and do not affect existing contract functionality. Existing integrations will continue to work unchanged.

The new `reclaim_asset` function provides a dedicated way to emit asset reclamation events, which can be called by landlords, tenants, or system administrators.
