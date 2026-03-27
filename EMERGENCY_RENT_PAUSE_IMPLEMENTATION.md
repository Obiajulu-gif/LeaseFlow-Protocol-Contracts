# Emergency Rent Pause Implementation

## Overview

This implementation addresses issue #19 by adding an Emergency Rent Pause feature for natural disasters and force majeure events. The feature allows authorized parties to pause rent accrual during emergencies, providing a social safety net for tenants facing circumstances beyond their control.

## Key Features

### 1. Emergency Pause Authority
- **Admin**: Protocol administrators can pause any lease
- **Landlord**: Property owners can pause their own leases
- **Arbitrators**: Whitelisted third-party arbitrators can pause leases they're authorized for

### 2. Pause State Management
- **Pause Tracking**: Records when pauses start, who initiated them, and the reason
- **Duration Tracking**: Accumulates total time spent in paused state
- **Status Changes**: Lease status changes to `LeaseStatus::Paused` during emergency

### 3. Rent Calculation Adjustments
- **Paused Period Exclusion**: Rent calculations exclude time spent in paused state
- **Late Fee Protection**: No late fees accrue during emergency pauses
- **Eviction Protection**: Eviction events are not triggered while paused

### 4. Flexible Termination
- **Early Termination**: Paused leases can be terminated before their end date
- **Proper Settlement**: Deposit settlement still required before termination

## Implementation Details

### New Data Structures

#### Enhanced LeaseInstance
```rust
pub struct LeaseInstance {
    // ... existing fields ...
    
    /// Emergency pause state for natural disasters or force majeure events
    pub paused: bool,
    /// Reason for the emergency pause
    pub pause_reason: Option<String>,
    /// Timestamp when the lease was paused
    pub paused_at: Option<u64>,
    /// Address that initiated the pause (admin, landlord, or trusted third party)
    pub pause_initiator: Option<Address>,
    /// Total time spent in paused state (to adjust rent calculations)
    pub total_paused_duration: u64,
}
```

#### New Lease Status
```rust
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
    Terminated,
    Paused,  // New status for emergency pauses
}
```

#### New Events
```rust
#[contractevent]
pub struct EmergencyRentPaused {
    pub lease_id: u64,
    pub initiator: Address,
    pub reason: String,
    pub paused_at: u64,
}

#[contractevent]
pub struct EmergencyRentResumed {
    pub lease_id: u64,
    pub initiator: Address,
    pub resumed_at: u64,
    pub total_paused_duration: u64,
}
```

#### New Error Types
```rust
pub enum LeaseError {
    // ... existing errors ...
    LeaseAlreadyPaused = 14,
    LeaseNotPaused = 15,
    InvalidPauseReason = 16,
}
```

### Core Functions

#### 1. Emergency Pause Rent
```rust
pub fn emergency_pause_rent(
    env: Env,
    lease_id: u64,
    caller: Address,
    reason: String,
) -> Result<(), LeaseError>
```

**Authorization**: Admin, landlord, or whitelisted arbitrator
**Functionality**:
- Validates caller authorization
- Checks lease is not already paused
- Validates pause reason is not empty
- Updates lease state to paused
- Records pause timestamp and initiator
- Emits `EmergencyRentPaused` event

#### 2. Emergency Resume Rent
```rust
pub fn emergency_resume_rent(
    env: Env,
    lease_id: u64,
    caller: Address,
) -> Result<(), LeaseError>
```

**Authorization**: Admin, landlord, or original pause initiator
**Functionality**:
- Validates caller authorization
- Checks lease is currently paused
- Calculates and accumulates pause duration
- Updates lease state to active
- Emits `EmergencyRentResumed` event

#### 3. Get Pause Status
```rust
pub fn get_pause_status(
    env: Env,
    lease_id: u64,
) -> Result<(bool, Option<String>, Option<u64>, u64), LeaseError>
```

**Returns**: Tuple containing:
- `paused: bool` - Current pause state
- `pause_reason: Option<String>` - Reason for pause
- `paused_at: Option<u64>` - Pause timestamp
- `total_paused_duration: u64` - Total time paused

### Modified Functions

#### Enhanced Rent Calculation
The `check_tenant_default` function now accounts for paused periods:

```rust
// Calculate effective elapsed time (excluding paused periods)
let mut effective_elapsed_secs = current_time.saturating_sub(lease.start_date);

// Subtract total paused duration from elapsed time
effective_elapsed_secs = effective_elapsed_secs.saturating_sub(lease.total_paused_duration);

// If currently paused, subtract current pause duration
if lease.paused {
    if let Some(paused_at) = lease.paused_at {
        let current_pause_duration = current_time.saturating_sub(paused_at);
        effective_elapsed_secs = effective_elapsed_secs.saturating_sub(current_pause_duration);
    }
}
```

#### Protected Late Fees and Evictions
- Late fees only accrue when not paused
- Eviction events are not triggered during pauses
- Provides protection during emergency situations

#### Flexible Lease Termination
The `terminate_lease` function now allows termination of paused leases:

```rust
// Allow termination of paused leases or expired leases
let current_time = env.ledger().timestamp();
if !lease.paused && current_time < lease.end_date { 
    return Err(LeaseError::LeaseNotExpired); 
}
```

## Use Cases

### 1. Natural Disasters
```rust
// Flood makes property uninhabitable
client.emergency_pause_rent(
    &lease_id, 
    &admin, 
    &String::from_str(&env, "Flood damage - property uninhabitable")
);
```

### 2. Government Mandates
```rust
// Earthquake building condemnation
client.emergency_pause_rent(
    &lease_id, 
    &landlord, 
    &String::from_str(&env, "Earthquake - building condemned")
);
```

### 3. Pandemic Restrictions
```rust
// Mandatory evacuation order
client.emergency_pause_rent(
    &lease_id, 
    &arbitrator, 
    &String::from_str(&env, "Hurricane - mandatory evacuation")
);
```

## Security Considerations

### Authorization Model
- **Multi-tier Authorization**: Admin, landlord, and arbitrator roles
- **Signature Requirements**: All pause/resume actions require caller authentication
- **Audit Trail**: Complete history of pause actions with timestamps and initiators

### Abuse Prevention
- **Reason Validation**: Empty or invalid reasons are rejected
- **State Validation**: Cannot pause already paused leases
- **Resume Authorization**: Only authorized parties can resume rent

### Data Integrity
- **Duration Tracking**: Accurate calculation of paused time
- **State Consistency**: Proper lease status management
- **Event Logging**: Complete audit trail through events

## Testing

The implementation includes comprehensive tests covering:

1. **Authorization Tests**:
   - Admin can pause rent
   - Landlord can pause rent
   - Arbitrator can pause rent
   - Unauthorized users cannot pause rent

2. **State Management Tests**:
   - Cannot pause already paused lease
   - Cannot resume non-paused lease
   - Proper status transitions

3. **Rent Calculation Tests**:
   - Rent doesn't accrue during pause
   - Accurate calculation after resume
   - Duration tracking works correctly

4. **Termination Tests**:
   - Can terminate paused leases
   - Proper cleanup and archival

## Benefits

### For Tenants
- **Financial Protection**: No rent accrual during uninhabitable conditions
- **Eviction Protection**: Cannot be evicted during emergencies
- **Fair Treatment**: Accounts for circumstances beyond tenant control

### For Landlords
- **Flexibility**: Can pause rent for legitimate emergencies
- **Relationship Preservation**: Maintains good tenant relationships
- **Risk Management**: Proper handling of force majeure events

### For Protocol
- **Social Responsibility**: Demonstrates care for user welfare
- **Regulatory Compliance**: Meets expectations for responsible DeFi
- **Competitive Advantage**: Differentiates from purely automated systems

## Conclusion

The Emergency Rent Pause feature successfully addresses the need for a social safety net in decentralized property management. It provides the flexibility to handle real-world crises with empathy while maintaining the security and transparency benefits of blockchain technology.

The implementation is production-ready with comprehensive testing, proper authorization controls, and complete audit trails. It demonstrates how DeFi protocols can be both automated and humane, handling edge cases that pure algorithmic approaches cannot address effectively.