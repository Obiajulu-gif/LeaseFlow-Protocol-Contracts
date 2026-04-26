# Dynamic NFT Metadata Updates via Condition Oracles

## Summary
This PR implements Issue #106, bridging the physical state of leased assets with their tokenized representations through oracle-signed metadata updates.

## Problem Solved
Previously, if a leased car was crashed, the underlying smart contract lacked a mechanism to dynamically flag the asset's tokenized state. This created a disconnect between physical asset condition and on-chain representation, potentially allowing damaged assets to be sold at mint-condition prices on secondary markets.

## Solution Overview
Implemented a comprehensive `update_asset_condition_metadata` function that:
- Accepts cryptographically signed payloads from whitelisted condition Oracles
- Updates asset condition states (Mint → Worn → Damaged → Destroyed)
- Executes cross-contract calls to external Asset Registry
- Automatically triggers lease termination and full deposit slashing for catastrophic damage
- Emits events for off-chain indexers and NFT marketplaces

## Key Features

### 🔄 Asset Condition States
```rust
pub enum AssetCondition {
    Mint,     // Perfect condition
    Worn,     // Normal wear and tear
    Damaged,  // Significant damage
    Destroyed, // Total loss
}
```

### 🔐 Oracle Authentication
- Cryptographic signature verification using ED25519
- Nonce-based replay protection
- Whitelist-only oracle access
- Staleness detection (48-hour threshold)

### ⚡ Rate Limiting
- 1 update per hour per oracle maximum
- Prevents spamming the registry with micro-changes
- Protects against ledger bloat

### 🏛️ Cross-Contract Integration
- Seamless integration with external Asset Registry contracts
- Real-time metadata URI updates
- Maintains data consistency across protocols

### 🚨 Automatic Termination
- "Destroyed" condition triggers immediate lease termination
- Full deposit slashing for catastrophic damage
- Prevents continued payments on destroyed assets

### 📢 Event Emissions
```rust
pub struct AssetMetadataUpdated {
    pub lease_id: u64,
    pub asset_condition: AssetCondition,
    pub oracle_pubkey: BytesN<32>,
    pub update_timestamp: u64,
    pub previous_condition: AssetCondition,
}
```

## Security Considerations

### ✅ Implemented Protections
1. **Oracle Authentication**: Only whitelisted oracles can submit updates
2. **Signature Verification**: Cryptographic validation of all payloads
3. **Rate Limiting**: Prevents spam and potential DoS attacks
4. **Nonce Validation**: Prevents replay attacks
5. **Staleness Detection**: Rejects outdated reports
6. **Fallback Hierarchy**: Integrates with existing oracle fallback system

### 🛡️ Market Protection
- Real-time condition updates prevent selling damaged assets at mint prices
- Off-chain indexers receive immediate notifications
- NFT marketplaces can adjust pricing based on verified condition

## Testing Coverage

### 🧪 Comprehensive Test Suite
- ✅ Successful metadata updates
- ✅ Unauthorized oracle rejection
- ✅ Rate limiting enforcement
- ✅ Automatic termination on destruction
- ✅ Invalid nonce validation
- ✅ Stale timestamp rejection
- ✅ Cross-contract call verification

### 📊 Test Results
All tests pass, demonstrating:
- Correct oracle authentication flow
- Proper rate limiting behavior
- Accurate condition state transitions
- Reliable automatic termination logic

## API Usage

### Update Asset Condition
```rust
let payload = AssetConditionMetadataPayload {
    lease_id: 123,
    oracle_pubkey: oracle_pubkey,
    asset_condition: AssetCondition::Damaged,
    nonce: 1,
    timestamp: current_time,
    signature: signed_signature,
};

LeaseContract::update_asset_condition_metadata(
    env,
    payload,
    asset_registry_address,
)?;
```

### Get Current Condition
```rust
let condition = LeaseContract::get_asset_condition(env, lease_id);
```

## Acceptance Criteria Met

### ✅ Acceptance 1: Real-time Asset Reflection
- Tokenized representation accurately reflects physical condition
- Immediate updates via oracle reports
- Cross-contract synchronization with Asset Registry

### ✅ Acceptance 2: Catastrophic Damage Handling
- Automatic lease termination for "Destroyed" condition
- Full deposit slashing protocol execution
- Secure oracle verification prevents false reports

### ✅ Acceptance 3: Marketplace Protection
- Real-time event emissions for off-chain indexers
- Condition metadata accessible to NFT marketplaces
- Prevents selling damaged assets at mint-condition prices

## Integration Notes

### Oracle Setup
1. Whitelist oracle public keys via `whitelist_oracle()`
2. Configure fallback hierarchy if desired
3. Ensure oracle maintains proper nonce sequence

### Asset Registry Integration
- Implement `AssetRegistryInterface` in your registry contract
- Handle `update_asset_metadata` calls appropriately
- Maintain metadata URI consistency

### Event Monitoring
- Listen for `AssetMetadataUpdated` events
- Update off-chain pricing algorithms
- Notify marketplace integrators

## Breaking Changes
None. This is a pure additive feature that maintains backward compatibility.

## Future Enhancements
- Support for multiple condition severity levels
- Integration with IoT sensors for automated reporting
- Historical condition tracking for analytics
- Marketplace-specific metadata formatting

---

**Resolves**: #106  
**Labels**: oracle, nft, data, metadata
