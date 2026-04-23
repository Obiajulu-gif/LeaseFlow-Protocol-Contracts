#![cfg(test)]

use soroban_sdk::{
    contracterror, contracttype, symbol, testutils::Address as TestAddress, vec, Address, Env,
    Symbol,
};
use crate::{
    AssetTier, DepositStatus, LeaseContract, LeaseError, LeaseStatus, SecurityDeposit,
    EscrowVault, MultiAssetCollateral,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestToken {
    pub address: Address,
    pub decimals: u32,
}

impl TestToken {
    pub fn new(env: &Env, address: Address) -> Self {
        Self { address, decimals: 7 }
    }
}

fn create_test_env() -> Env {
    Env::default()
}

fn setup_test_contract(env: &Env) -> Address {
    let contract_address = env.register_contract(None, LeaseContract);
    contract_address
}

fn setup_admin(env: &Env, contract_address: &Address) -> Address {
    let admin = TestAddress::generate(env);
    // Set admin
    LeaseContract::set_admin(env.clone(), contract_address.clone(), admin.clone()).unwrap();
    admin
}

fn setup_test_token(env: &Env) -> Address {
    TestAddress::generate(env)
}

fn create_test_lease_params(
    env: &Env,
    tenant: Address,
    landlord: Address,
    payment_token: Address,
) -> crate::CreateLeaseParams {
    crate::CreateLeaseParams {
        tenant,
        rent_amount: 1000_0000i128, // 1000 tokens
        deposit_amount: 500_0000i128, // 500 tokens
        security_deposit: 200_0000i128, // 200 tokens
        start_date: env.ledger().timestamp(),
        end_date: env.ledger().timestamp() + 90 * 24 * 60 * 60, // 90 days
        property_uri: soroban_sdk::String::from_str(env, "test_property"),
        payment_token,
        arbitrators: vec![env, TestAddress::generate(env)],
        rent_per_sec: 0,
        grace_period_end: env.ledger().timestamp() + 90 * 24 * 60 * 60,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 500, // 5%
        swap_path: vec![env],
    }
}

#[test]
fn test_security_deposit_basic_flow() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 1_000_000_000_000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Deposit security collateral
    let deposit_amount = 200_0000i128; // 200 tokens
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        payment_token.clone(),
        deposit_amount,
        lease_duration,
        None, // No multi-asset collateral
    )
    .unwrap();

    // Verify security deposit was created
    let security_deposit = LeaseContract::get_security_deposit(env.clone(), lease_id).unwrap();
    assert_eq!(security_deposit.lease_id, lease_id);
    assert_eq!(security_deposit.lessee, tenant);
    assert_eq!(security_deposit.lessor, landlord);
    assert_eq!(security_deposit.asset_address, payment_token);
    assert_eq!(security_deposit.amount, deposit_amount);
    assert_eq!(security_deposit.status, DepositStatus::Held);
    assert_eq!(security_deposit.asset_tier, AssetTier::Medium);

    // Verify escrow vault state
    let vault = LeaseContract::get_escrow_vault(env.clone());
    assert_eq!(vault.total_locked, deposit_amount);
    assert_eq!(vault.lease_count, 1);
}

#[test]
fn test_security_deposit_wrong_amount_fails() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 1_000_000_000_000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Try to deposit wrong amount
    let wrong_deposit_amount = 150_0000i128; // Wrong amount
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    let result = LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        payment_token.clone(),
        wrong_deposit_amount,
        lease_duration,
        None, // No multi-asset collateral
    );

    assert_eq!(result.unwrap_err(), LeaseError::InvalidDeduction);
}

#[test]
fn test_security_deposit_tvl_exceeded() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set very low max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 100_0000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Try to deposit amount that exceeds TVL
    let deposit_amount = 200_0000i128; // 200 tokens > TVL of 100 tokens
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    let result = LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        payment_token.clone(),
        deposit_amount,
        lease_duration,
        None, // No multi-asset collateral
    );

    assert_eq!(result.unwrap_err(), LeaseError::EscrowCapacityExceeded);
}

#[test]
fn test_lessor_cannot_access_deposit_during_active_lease() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 1_000_000_000_000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Deposit security collateral
    let deposit_amount = 200_0000i128; // 200 tokens
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        payment_token.clone(),
        deposit_amount,
        lease_duration,
        None, // No multi-asset collateral
    )
    .unwrap();

    // Try to release deposit as lessor (should fail)
    let result = LeaseContract::release_security_deposit(
        env.clone(),
        lease_id,
        landlord.clone(), // Lessor trying to access
        0i128, // No damage deduction
    );

    assert_eq!(result.unwrap_err(), LeaseError::Unauthorised);
}

#[test]
fn test_100_concurrent_leases_isolated_mapping() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set high max protocol TVL for 100 leases
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 100_000_000_000_000i128).unwrap();

    let num_leases = 100u64;
    let deposit_amount = 200_0000i128; // 200 tokens per lease
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    // Create 100 concurrent leases
    for i in 1..=num_leases {
        let tenant = TestAddress::generate(&env);
        let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
        
        LeaseContract::create_lease_instance(
            env.clone(),
            i,
            landlord.clone(),
            lease_params,
        )
        .unwrap();

        // Deposit security collateral for each lease
        LeaseContract::deposit_security_collateral(
            env.clone(),
            i,
            tenant.clone(),
            payment_token.clone(),
            deposit_amount,
            lease_duration,
            None, // No multi-asset collateral
        )
        .unwrap();
    }

    // Verify all leases have isolated deposits
    for i in 1..=num_leases {
        let security_deposit = LeaseContract::get_security_deposit(env.clone(), i).unwrap();
        assert_eq!(security_deposit.lease_id, i);
        assert_eq!(security_deposit.amount, deposit_amount);
        assert_eq!(security_deposit.status, DepositStatus::Held);
        assert_eq!(security_deposit.asset_tier, AssetTier::Medium);
    }

    // Verify escrow vault state
    let vault = LeaseContract::get_escrow_vault(env.clone());
    assert_eq!(vault.total_locked, deposit_amount * num_leases as i128);
    assert_eq!(vault.lease_count, num_leases);

    // Verify each lease can be independently accessed
    for i in 1..=num_leases {
        let lease = LeaseContract::get_lease_instance(env.clone(), i).unwrap();
        assert_eq!(lease.security_deposit, deposit_amount);
        assert_eq!(lease.deposit_status, DepositStatus::Held);
    }
}

#[test]
fn test_multi_asset_collateral() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let primary_token = setup_test_token(&env);
    let secondary_token = setup_test_token(&env);

    // Set asset tiers
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        primary_token.clone(),
        AssetTier::High,
    )
    .unwrap();

    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        secondary_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 1_000_000_000_000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), primary_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Create multi-asset collateral
    let multi_asset_collateral = MultiAssetCollateral {
        primary_asset: primary_token.clone(),
        primary_amount: 300_0000i128, // 300 tokens
        secondary_asset: Some(secondary_token.clone()),
        secondary_amount: Some(100_0000i128), // 100 tokens
        nft_contract: None,
        nft_token_id: None,
    };

    // Deposit security collateral with multi-asset support
    let deposit_amount = 300_0000i128; // 300 tokens (primary)
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        primary_token.clone(),
        deposit_amount,
        lease_duration,
        Some(multi_asset_collateral),
    )
    .unwrap();

    // Verify security deposit was created with primary asset
    let security_deposit = LeaseContract::get_security_deposit(env.clone(), lease_id).unwrap();
    assert_eq!(security_deposit.asset_address, primary_token);
    assert_eq!(security_deposit.amount, deposit_amount);
    assert_eq!(security_deposit.asset_tier, AssetTier::High);

    // Verify escrow vault includes both assets
    let vault = LeaseContract::get_escrow_vault(env.clone());
    assert_eq!(vault.total_locked, 400_0000i128); // 300 + 100 tokens
    assert_eq!(vault.lease_count, 1);
}

#[test]
fn test_asset_tier_deposit_calculation() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Test different asset tiers
    let tiers = vec![
        (AssetTier::Low, 100_0000i128),      // Should be 0.5x = 50 tokens
        (AssetTier::Medium, 100_0000i128),   // Should be 1.0x = 100 tokens
        (AssetTier::High, 100_0000i128),     // Should be 2.0x = 200 tokens
        (AssetTier::Luxury, 100_0000i128),   // Should be 5.0x = 500 tokens
    ];

    for (i, (tier, base_amount)) in tiers.iter().enumerate() {
        let lease_id = (i + 1) as u64;

        // Set asset tier
        LeaseContract::set_asset_tier(
            env.clone(),
            admin.clone(),
            payment_token.clone(),
            tier.clone(),
        )
        .unwrap();

        // Create lease instance
        let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
        
        LeaseContract::create_lease_instance(
            env.clone(),
            lease_id,
            landlord.clone(),
            lease_params,
        )
        .unwrap();

        // Calculate expected deposit amount
        let lease_duration = 90 * 24 * 60 * 60u64; // 90 days = 1.5x multiplier
        let tier_multiplier = match tier {
            AssetTier::Low => 50,
            AssetTier::Medium => 100,
            AssetTier::High => 200,
            AssetTier::Luxury => 500,
        };
        let expected_amount = base_amount * 150 * tier_multiplier / 10_000; // duration 1.5x * tier multiplier

        // Deposit should succeed with exact amount
        let result = LeaseContract::deposit_security_collateral(
            env.clone(),
            lease_id,
            tenant.clone(),
            payment_token.clone(),
            expected_amount,
            lease_duration,
            None,
        );

        assert!(result.is_ok());

        // Verify deposit
        let security_deposit = LeaseContract::get_security_deposit(env.clone(), lease_id).unwrap();
        assert_eq!(security_deposit.amount, expected_amount);
        assert_eq!(security_deposit.asset_tier, *tier);
    }
}

#[test]
fn test_security_deposit_release_after_lease_termination() {
    let env = create_test_env();
    let contract_address = setup_test_contract(&env);
    let admin = setup_admin(&env, &contract_address);
    let landlord = TestAddress::generate(&env);
    let tenant = TestAddress::generate(&env);
    let payment_token = setup_test_token(&env);

    // Set asset tier for payment token
    LeaseContract::set_asset_tier(
        env.clone(),
        admin.clone(),
        payment_token.clone(),
        AssetTier::Medium,
    )
    .unwrap();

    // Set max protocol TVL
    LeaseContract::set_max_protocol_tvl(env.clone(), admin.clone(), 1_000_000_000_000i128).unwrap();

    // Create lease instance
    let lease_params = create_test_lease_params(&env, tenant.clone(), landlord.clone(), payment_token.clone());
    let lease_id = 1u64;

    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Deposit security collateral
    let deposit_amount = 200_0000i128; // 200 tokens
    let lease_duration = 90 * 24 * 60 * 60u64; // 90 days

    LeaseContract::deposit_security_collateral(
        env.clone(),
        lease_id,
        tenant.clone(),
        payment_token.clone(),
        deposit_amount,
        lease_duration,
        None, // No multi-asset collateral
    )
    .unwrap();

    // Simulate lease termination by updating lease status
    let mut lease = LeaseContract::get_lease_instance(env.clone(), lease_id).unwrap();
    lease.status = LeaseStatus::Terminated;
    crate::save_lease_instance(&env, lease_id, &lease);

    // Release security deposit with no damage
    let refund_amount = LeaseContract::release_security_deposit(
        env.clone(),
        lease_id,
        tenant.clone(), // Lessee can release after termination
        0i128, // No damage deduction
    )
    .unwrap();

    assert_eq!(refund_amount, deposit_amount);

    // Verify deposit status
    let security_deposit = LeaseContract::get_security_deposit(env.clone(), lease_id).unwrap();
    assert_eq!(security_deposit.status, DepositStatus::Settled);

    // Verify vault state updated
    let vault = LeaseContract::get_escrow_vault(env.clone());
    assert_eq!(vault.total_locked, 0);
    assert_eq!(vault.lease_count, 0);
}
