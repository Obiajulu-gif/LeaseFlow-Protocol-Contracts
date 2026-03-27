# Auto-Pay Implementation for LeaseFlow Smart Contract

## Overview

This implementation adds an "Auto-Pay" mechanism to the LeaseFlow smart contract, allowing tenants to authorize automatic rent withdrawals every billing cycle. This mimics the convenience of traditional auto-pay systems while maintaining the security and transparency of blockchain technology.

## Key Features

### 1. Tenant Authorization
- Tenants can authorize the contract to automatically withdraw a specific amount
- Authorization includes the amount and billing cycle duration
- Default billing cycle is 30 days (2,592,000 seconds)
- Custom billing cycles are supported (e.g., weekly, bi-weekly)

### 2. Landlord Execution
- Landlords can execute rent pulls once per billing cycle
- Automatic validation ensures pulls don't exceed authorized amounts
- Rent calculations are based on the lease's `rent_per_sec` rate

### 3. Security & Control
- Only tenants can authorize/revoke their own auto-pay
- Only landlords can execute rent pulls for their properties
- Billing cycle enforcement prevents multiple pulls within the same period
- Authorization can be revoked at any time by the tenant

## Implementation Details

### Data Structure Changes

#### LeaseInstance Struct
Added three new fields to track auto-pay state:

```rust
pub struct LeaseInstance {
    // ... existing fields ...
    
    /// Amount approved for automatic withdrawal per billing cycle
    pub rent_pull_authorized_amount: Option<i128>,
    
    /// Timestamp of the last rent pull execution
    pub last_rent_pull_timestamp: Option<u64>,
    
    /// Billing cycle duration in seconds (default: 30 days)
    pub billing_cycle_duration: u64,
}
```

#### New Error Types
```rust
pub enum LeaseError {
    // ... existing errors ...
    RentPullNotAuthorized = 17,
    BillingCycleNotElapsed = 18,
    InsufficientAuthorizedAmount = 19,
}
```

#### New Events
```rust
#[contractevent]
pub struct RentPullAuthorized {
    pub lease_id: u64,
    pub tenant: Address,
    pub authorized_amount: i128,
    pub billing_cycle_duration: u64,
}

#[contractevent]
pub struct RentPullExecuted {
    pub lease_id: u64,
    pub landlord: Address,
    pub amount_pulled: i128,
    pub timestamp: u64,
}

#[contractevent]
pub struct RentPullRevoked {
    pub lease_id: u64,
    pub tenant: Address,
    pub timestamp: u64,
}
```

### Core Functions

#### 1. `authorize_rent_pull`
```rust
pub fn authorize_rent_pull(
    env: Env,
    lease_id: u64,
    tenant: Address,
    authorized_amount: i128,
    billing_cycle_duration: Option<u64>,
) -> Result<(), LeaseError>
```

**Purpose**: Allows tenants to authorize automatic rent withdrawals.

**Parameters**:
- `lease_id`: Unique identifier of the lease
- `tenant`: Tenant address (must match lease tenant)
- `authorized_amount`: Maximum amount that can be pulled per billing cycle
- `billing_cycle_duration`: Optional custom billing cycle (defaults to 30 days)

**Security**: Only the lease tenant can call this function.

#### 2. `execute_rent_pull`
```rust
pub fn execute_rent_pull(
    env: Env,
    lease_id: u64,
    landlord: Address,
    token_contract_id: Address,
) -> Result<i128, LeaseError>
```

**Purpose**: Executes an automatic rent withdrawal.

**Parameters**:
- `lease_id`: Unique identifier of the lease
- `landlord`: Landlord address (must match lease landlord)
- `token_contract_id`: Payment token contract address

**Validation**:
- Checks if rent pull is authorized
- Ensures billing cycle has elapsed since last pull
- Verifies authorized amount covers required rent
- Calculates rent based on `rent_per_sec * billing_cycle_duration`

**Security**: Only the lease landlord can call this function.

#### 3. `revoke_rent_pull_authorization`
```rust
pub fn revoke_rent_pull_authorization(
    env: Env,
    lease_id: u64,
    tenant: Address,
) -> Result<(), LeaseError>
```

**Purpose**: Allows tenants to revoke auto-pay authorization.

**Security**: Only the lease tenant can call this function.

#### 4. `get_rent_pull_status`
```rust
pub fn get_rent_pull_status(
    env: Env,
    lease_id: u64,
) -> Result<(Option<i128>, Option<u64>, u64, Option<u64>), LeaseError>
```

**Purpose**: Returns current auto-pay status for a lease.

**Returns**:
- `authorized_amount`: Amount authorized for auto-pull (None if not authorized)
- `last_pull_timestamp`: Timestamp of last successful pull
- `billing_cycle_duration`: Duration of billing cycle in seconds
- `next_pull_available`: Timestamp when next pull becomes available

## Usage Examples

### 1. Basic Monthly Auto-Pay Setup

```rust
// Tenant authorizes 1000 USDC monthly auto-pay
let authorized_amount = 1000_0000000; // 1000 USDC (7 decimals)
client.authorize_rent_pull(&lease_id, &tenant, &authorized_amount, &None);

// Landlord executes monthly rent pull
let pulled_amount = client.execute_rent_pull(&lease_id, &landlord, &usdc_token);
```

### 2. Weekly Auto-Pay Setup

```rust
// Tenant authorizes 250 USDC weekly auto-pay
let authorized_amount = 250_0000000; // 250 USDC
let weekly_cycle = Some(7 * 24 * 60 * 60u64); // 7 days in seconds
client.authorize_rent_pull(&lease_id, &tenant, &authorized_amount, &weekly_cycle);
```

### 3. Checking Auto-Pay Status

```rust
let (auth_amount, last_pull, cycle_duration, next_available) = 
    client.get_rent_pull_status(&lease_id);

if let Some(amount) = auth_amount {
    println!("Auto-pay authorized for {} tokens", amount);
    if let Some(next) = next_available {
        println!("Next pull available at timestamp {}", next);
    }
}
```

### 4. Revoking Auto-Pay

```rust
// Tenant revokes auto-pay authorization
client.revoke_rent_pull_authorization(&lease_id, &tenant);
```

## Security Considerations

### 1. Authorization Control
- Only tenants can authorize auto-pay for their own leases
- Only landlords can execute pulls for their own properties
- Authorization can be revoked at any time by the tenant

### 2. Amount Validation
- Authorized amount must cover the calculated rent for the billing cycle
- Rent calculation: `rent_per_sec * billing_cycle_duration`
- The system pulls the minimum of authorized amount or calculated rent

### 3. Timing Protection
- Billing cycle enforcement prevents multiple pulls within the same period
- Timestamps are managed by the blockchain ledger for accuracy
- No pull can occur before the billing cycle has elapsed

### 4. Token Security
- Uses Soroban's standard token interface for transfers
- Transfers are atomic and fail-safe
- All transfers are logged via events for transparency

## Integration with Existing Features

### 1. Emergency Pause Compatibility
- Auto-pay continues to work during emergency pauses
- Tenants can still pay rent to stay current during emergencies
- Rent calculations account for paused periods

### 2. Maintenance Workflow Integration
- Auto-pay respects maintenance status
- Payments during maintenance issues are held as `withheld_rent`
- Normal auto-pay resumes after maintenance verification

### 3. Late Fee Handling
- Auto-pay helps prevent late fees by ensuring timely payments
- If auto-pay fails, normal late fee calculations apply
- Grace periods are still respected

## Testing

The implementation includes comprehensive tests covering:

1. **Authorization Tests**
   - Successful authorization by tenant
   - Unauthorized authorization attempts
   - Custom billing cycle settings

2. **Execution Tests**
   - Successful rent pulls
   - Billing cycle enforcement
   - Insufficient authorization handling
   - Unauthorized execution attempts

3. **Revocation Tests**
   - Successful revocation by tenant
   - Unauthorized revocation attempts

4. **Status Query Tests**
   - Authorization status tracking
   - Next pull availability calculation
   - Multiple billing cycle handling

## Benefits

### For Tenants
- **Convenience**: Automatic rent payments without manual intervention
- **Control**: Can authorize, modify, or revoke at any time
- **Flexibility**: Custom billing cycles (weekly, bi-weekly, monthly)
- **Transparency**: All transactions are recorded on-chain

### For Landlords
- **Predictable Income**: Regular, automated rent collection
- **Reduced Administrative Burden**: No need to chase payments
- **Immediate Execution**: Can pull rent as soon as billing cycle allows
- **Transparency**: Clear audit trail of all payments

### For the Protocol
- **Increased Adoption**: Familiar auto-pay functionality
- **Reduced Disputes**: Automated, transparent payment system
- **Better Cash Flow**: More consistent rent payments
- **Enhanced User Experience**: Modern payment convenience

## Future Enhancements

1. **Partial Payment Handling**: Support for partial auto-pay amounts
2. **Payment Scheduling**: More granular scheduling options
3. **Multi-Token Support**: Auto-pay with different token types
4. **Payment Notifications**: Off-chain notification system
5. **Backup Payment Methods**: Fallback payment sources

## Conclusion

The Auto-Pay implementation successfully brings modern payment convenience to the LeaseFlow protocol while maintaining the security, transparency, and decentralization benefits of blockchain technology. The system is designed to be secure, flexible, and user-friendly, providing value to both tenants and landlords in the decentralized rental ecosystem.