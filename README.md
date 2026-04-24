# LeaseFlow Protocol Contracts

Soroban smart contracts for real-time asset leasing with built-in grace period and delinquency management.

## Overview

LeaseFlow manages the full lifecycle of a property lease on-chain — from creation through rent streaming to deposit settlement and lease closure. This implementation includes critical grace period functionality to handle temporary liquidity issues and prevent unfair evictions.

## Features

### Core Functionality
- **Lease Creation**: Create leases with customizable terms (rent, deposit, duration)
- **Lease Activation**: Lessee can activate lease by depositing security deposit
- **Rent Processing**: Continuous rent payment streaming and processing
- **Deposit Management**: Security deposit handling and final settlement

### Grace Period & Delinquency Management (Issue #109)
- **Grace Period State**: Automatic transition to grace period on payment failure
- **Late Fee Calculation**: Configurable late fee rates (basis points)
- **Recovery Logic**: Lease recovery during grace period with fee appending
- **Eviction Protection**: Structured transition to eviction pending after grace period expiry
- **Event Emission**: Comprehensive events for external notification systems

### Security Features
- **Time-based Protection**: Grace period math resistant to time manipulation
- **Access Control**: Role-based authorization for lease operations
- **State Validation**: Strict state machine transitions
- **Error Handling**: Comprehensive error types for debugging

## Contract Structure

```
contracts/leaseflow/
├── src/
│   ├── lib.rs          # Main contract implementation
│   └── test.rs         # Comprehensive test suite
└── Cargo.toml          # Contract dependencies
```

## Lease States

```rust
pub enum LeaseState {
    Pending,        // Lease created, waiting for activation
    Active,         // Lease active, rent payments flowing
    GracePeriod,    // Payment failed, grace period active
    EvictionPending,// Grace period expired, eviction pending
    Closed,         // Lease completed and closed
}
```

## Key Functions

### Lease Management
- `create_lease()` - Create new lease with terms
- `activate_lease()` - Activate lease with deposit
- `get_lease()` - Retrieve lease information
- `get_user_leases()` - Get all leases for a user

### Payment Processing
- `process_rent_payment()` - Process rent payment (normal or recovery)
- `handle_rent_payment_failure()` - Trigger grace period on payment failure

### Grace Period Management
- `check_grace_period_expiry()` - Check and handle grace period expiration
- `trigger_grace_period_check()` - Manual grace period check (lessor only)

## Events

### Lease Lifecycle Events
- `LeaseCreated` - Emitted when lease is created
- `LeaseActivated` - Emitted when lease becomes active

### Grace Period Events
- `RentDelinquencyStarted` - Emitted when grace period begins
- `LeaseRecovered` - Emitted when lease recovers from delinquency
- `EvictionPending` - Emitted when grace period expires

## Usage Example

```rust
// Create lease
let lease_id = contract.create_lease(
    &lessor,
    &lessee,
    &1000,           // rent amount
    &2000,           // deposit amount
    &start_date,
    &end_date,
    &432000,         // 5 day grace period
    &500,            // 5% late fee rate
    &property_uri,
);

// Activate lease
contract.activate_lease(&lease_id, &lessee);

// Process rent payment
contract.process_rent_payment(&lease_id, &1000);

// Handle payment failure (triggers grace period)
contract.handle_rent_payment_failure(&lease_id);

// Recover during grace period (rent + late fees)
contract.process_rent_payment(&lease_id, &1050);
```

## Grace Period Flow

1. **Payment Failure**: Rent payment fails → `Error::InsufficientRentFunds`
2. **Grace Period Activation**: Lease transitions to `GracePeriod` state
3. **Late Fee Calculation**: Late fees calculated and accumulated
4. **Event Emission**: `RentDelinquencyStarted` event emitted
5. **Recovery Window**: Lessee has `MAX_GRACE_PERIOD` to pay outstanding amount + fees
6. **Recovery**: Full payment → `LeaseRecovered` event → return to `Active` state
7. **Expiry**: Grace period expires → `EvictionPending` event → `EvictionPending` state

## Configuration

### Grace Period
- **Default**: 5 days (432,000 seconds)
- **Configurable**: Set per lease during creation
- **Security**: Resistant to timestamp manipulation

### Late Fees
- **Rate**: Configurable in basis points (10000 = 100%)
- **Calculation**: `rent_amount * (late_fee_rate / 10000)`
- **Examples**:
  - 500 basis points = 5% late fee
  - 1000 basis points = 10% late fee
  - 0 basis points = no late fee

## Testing

Run the comprehensive test suite:

```bash
cargo test --package leaseflow
```

### Test Coverage
- ✅ Lease creation and activation
- ✅ Grace period triggering and state transitions
- ✅ Late fee calculation and edge cases
- ✅ Recovery during grace period
- ✅ Grace period expiry and eviction transition
- ✅ Event emission verification
- ✅ Authorization and access control
- ✅ Multiple lease management
- ✅ Wallet depletion simulation

## Security Considerations

### Time Manipulation Resistance
- Grace period calculations use ledger timestamps
- No timezone-based calculations
- Immutable grace period duration

### Access Control
- Only lessee can activate lease
- Only lessor can trigger manual grace period checks
- State transition validation prevents unauthorized operations

### Financial Safety
- All payments require sufficient funds
- Late fees calculated using safe math operations
- Overflow protection in all calculations

## Integration

### External Services
The contract emits events that can be consumed by:
- Email notification systems
- SMS alert services
- Dashboard monitoring
- Automated payment processors

### Payment Streams
Designed to integrate with continuous payment streams:
- Stellar payment streams
- Automated recurring payments
- Wallet balance monitoring

## Development

### Building
```bash
cargo build --package leaseflow --target wasm32-unknown-unknown
```

### Testing
```bash
cargo test --package leaseflow
```

### Deployment
Deploy to Stellar Testnet:
```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/leaseflow.wasm \
  --source-account <ACCOUNT> \
  --network testnet
```

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit pull request

## Issues

For bug reports and feature requests, please use the GitHub issue tracker.

---

**Note**: This implementation addresses Issue #109 - Missed Payment Grace Period & Dunning State, providing a robust solution for handling temporary liquidity issues while protecting both lessor and lessee interests.
