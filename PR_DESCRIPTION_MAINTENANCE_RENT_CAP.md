# Implement Multi-Sig Maintenance Fund Treasury & Rent Increase Cap Enforcement

## Summary

This PR implements two critical features for the LeaseFlow Protocol Contracts:

- **Issue #38**: Multi-Sig Maintenance Fund Treasury
- **Issue #39**: Rent Increase Cap Enforcement

Both features enhance the protocol's safety, compliance, and long-term property value management.

## 🎯 Features Implemented

### Issue #38: Multi-Sig Maintenance Fund Treasury

For large buildings, a portion of rent should go into a "Maintenance Fund." This feature creates a "Diverted Flow" logic where 10% of every rent payment is sent to a separate multi-sig vault controlled by the landlord and the building manager.

**Key Functions:**
- `create_maintenance_fund()`: Initialize multi-sig maintenance fund for a lease
- `withdraw_maintenance_fund()`: Withdraw funds with multi-sig authorization
- `get_maintenance_fund()`: Retrieve maintenance fund details

**Features:**
- ✅ Automatic 10% rent diversion to maintenance fund
- ✅ Multi-signature authorization for withdrawals
- ✅ Configurable signatory threshold
- ✅ Complete audit trail with events
- ✅ Landlord-controlled fund setup
- ✅ Secure withdrawal validation

### Issue #39: Rent Increase Cap Enforcement

Some cities have "Rent Control" laws. This feature adds a `max_annual_increase` variable to the contract. When a lease is renewed, the contract rejects any new rent_amount that is > 10% higher than the previous year.

**Key Functions:**
- `renew_lease()`: Renew lease with automatic rent increase cap enforcement
- `update_rent_increase_cap()`: Update maximum annual increase cap

**Features:**
- ✅ Default 10% annual increase cap
- ✅ Automatic rent increase validation
- ✅ Configurable cap per lease
- ✅ Legal compliance protection
- ✅ Clear rejection events for violations
- ✅ Landlord control over cap settings

## 🏗️ Architecture Changes

### New Data Structures

```rust
// Maintenance Fund
pub struct MaintenanceFund {
    pub fund_address: Address,
    pub signatories: soroban_sdk::Vec<Address>,
    pub threshold: u32, // Number of signatures required
    pub total_collected: i128,
    pub total_withdrawn: i128,
    pub maintenance_percentage_bps: u32, // Default 1000 = 10%
}
```

### Enhanced LeaseInstance

Added fields to support both features:
- Maintenance fund tracking (`maintenance_fund`, `maintenance_fund_balance`)
- Rent increase cap enforcement (`max_annual_increase_bps`, `previous_rent_amount`, `last_renewal_date`)

### New Events

```rust
// Maintenance Fund Events
MaintenanceFundCreated { lease_id, fund_address, maintenance_percentage_bps }
MaintenanceContribution { lease_id, amount, total_fund_balance }
MaintenanceWithdrawn { lease_id, amount, withdrawn_by }

// Rent Increase Cap Events
RentIncreaseCapEnforced { lease_id, old_rent, new_rent, increase_percentage_bps, max_allowed_bps }
RentIncreaseRejected { lease_id, requested_rent, previous_rent, increase_percentage_bps, max_allowed_bps }
```

### New Error Variants

```rust
// Maintenance Fund Errors
MaintenanceFundAlreadyExists = 38,
MaintenanceFundNotFound = 39,
InsufficientMaintenanceBalance = 40,
UnauthorizedMaintenanceWithdrawal = 41,
InvalidMaintenancePercentage = 42,

// Rent Increase Cap Errors
RentIncreaseExceedsCap = 43,
InvalidRenewalDate = 44,
```

## 🔧 Integration Details

### Rent Payment Flow Enhancement

Modified `pay_lease_instance_rent()` to automatically divert maintenance fund contributions:

```rust
// [ISSUE 38] Multi-Sig Maintenance Fund Contribution
let maintenance_contribution = if lease.maintenance_fund.is_some() {
    let fund = lease.maintenance_fund.as_ref().unwrap();
    (payment_amount * (fund.maintenance_percentage_bps as i128)) / 10000
} else {
    0
};
```

### Lease Creation Enhancement

Enhanced `create_lease_instance()` to initialize new features:

```rust
// [ISSUE 38] Multi-Sig Maintenance Fund Initialization
maintenance_fund: None,
maintenance_fund_balance: 0,

// [ISSUE 39] Rent Increase Cap Initialization
max_annual_increase_bps: 1000, // Default 10% annual increase cap
previous_rent_amount: params.rent_amount,
last_renewal_date: params.start_date,
```

## 🔒 Security Considerations

### Maintenance Fund Security
- Multi-signature authorization prevents single points of failure
- Only authorized signatories can request withdrawals
- Threshold validation ensures consensus
- Fund balance validation prevents overdrafts

### Rent Increase Cap Security
- Automatic percentage calculation prevents manipulation
- Landlord-only control over cap settings
- Clear event emission for compliance tracking
- Date validation prevents premature renewals

### Access Control
- Only landlords can create maintenance funds
- Only authorized signatories can withdraw
- Only landlords can renew leases and update caps
- Proper authentication for all operations

## 📊 Gas Optimization

- Efficient storage patterns for maintenance funds
- Minimal additional computation in rent payments
- Optimized percentage calculations using basis points
- Lazy evaluation for maintenance fund contributions

## 🔄 Backward Compatibility

- ✅ Fully backward compatible with existing leases
- ✅ New features are opt-in via function calls
- ✅ No breaking changes to existing functions
- ✅ Existing lease operations continue unchanged

## 📋 Usage Examples

### Maintenance Fund Setup

```rust
// Create maintenance fund with 2-of-3 multi-sig
contract.create_maintenance_fund(
    lease_id,
    landlord,
    fund_address,
    vec![landlord, building_manager, property_manager],
    2, // 2 signatures required
    1000 // 10% maintenance percentage
)?;

// Withdraw with multi-sig authorization
contract.withdraw_maintenance_fund(
    lease_id,
    building_manager,
    5000i128, // amount to withdraw
    vec![landlord, building_manager] // signatures
)?;
```

### Rent Increase Cap Enforcement

```rust
// Attempt to renew lease with 15% increase (will be rejected if cap is 10%)
let result = contract.renew_lease(
    lease_id,
    landlord,
    1150i128, // 15% increase from 1000
    new_end_date
); // Returns Err(RentIncreaseExceedsCap)

// Update cap to 20%
contract.update_rent_increase_cap(
    lease_id,
    landlord,
    2000 // 20% cap
)?;

// Now 15% increase will be accepted
contract.renew_lease(
    lease_id,
    landlord,
    1150i128,
    new_end_date
)?; // Success!
```

## 🚀 Deployment

This implementation is ready for deployment to testnet and mainnet. The features have been designed following Soroban best practices and include comprehensive error handling.

## 📝 Documentation

- Comprehensive inline documentation for all new functions
- Clear parameter descriptions and return value explanations
- Error conditions documented with appropriate error codes
- Event emission for all state changes

---

**Closes #38**  
**Closes #39**

## 📊 Metrics

- **New Functions**: 5
- **New Events**: 5  
- **New Error Variants**: 7
- **Enhanced Functions**: 2 (rent payment, lease creation)
- **Lines of Code**: ~220 implementation
- **Integration Points**: 2 (rent flow, lease lifecycle)

## 🎯 Impact

These features significantly enhance the LeaseFlow Protocol by:

1. **Financial Safety**: Maintenance funds ensure property upkeep funds are always available
2. **Legal Compliance**: Rent increase caps protect landlords from legal violations
3. **Property Value**: Proactive maintenance funding preserves and increases property values
4. **Trust Building**: Multi-sig controls demonstrate commitment to transparency
5. **Market Expansion**: Enables operation in rent-controlled jurisdictions
