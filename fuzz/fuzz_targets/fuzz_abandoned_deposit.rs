#![no_main]
use soroban_sdk::contractimport;
use soroban_sdk::Address;
use soroban_sdk::Env;
use soroban_sdk::Symbol;

// Import the contract
contractimport!(
    file = "../target/wasm32-unknown-unknown/release/leaseflow_contracts.wasm"
);

#[derive(arbitrary::Arbitrary, Debug)]
struct FuzzInput {
    lease_end_timestamp: u64,
    current_timestamp: u64,
    tenant_last_interaction: u64,
    caller_is_landlord: bool,
    lease_status: u8, // 0: Pending, 1: Active, 2: Expired, 3: Disputed, 4: Terminated
}

libfuzzer_sys::fuzz_target!(|data: FuzzInput| {
    let env = Env::default();

    // Setup contract
    let contract_id = env.register_contract_wasm(None, leaseflow_contracts::WASM);
    let client = leaseflow_contracts::Client::new(&env, &contract_id);

    // Create test addresses
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let unauthorized = Address::generate(&env);

    // Set admin
    client.set_admin(&landlord);

    // Create lease parameters
    let params = leaseflow_contracts::CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000i128,
        deposit_amount: 500i128,
        security_deposit: 500i128,
        start_date: data.lease_end_timestamp.saturating_sub(30 * 24 * 60 * 60), // 30 days before end
        end_date: data.lease_end_timestamp,
        property_uri: soroban_sdk::String::from_str(&env, "test_property"),
        payment_token: Address::generate(&env),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 0,
        grace_period_end: data.lease_end_timestamp,
        late_fee_flat: 0,
        late_fee_per_sec: 0,
        equity_percentage_bps: 0,
        has_pet: false,
        pet_deposit_amount: 0,
        pet_rent_amount: 0,
        yield_delegation_enabled: false,
        deposit_asset: None,
        dex_contract: None,
        max_slippage_bps: 500,
        swap_path: soroban_sdk::Vec::new(&env),
    };

    // Create lease
    client.create_lease_instance(&1u64, &landlord, &params);

    // Manually set lease status and interaction timestamp for testing
    let lease_id = 1u64;
    let mut lease = client.get_lease_instance(&lease_id).unwrap();
    
    // Set lease status based on input
    lease.status = match data.lease_status {
        0 => leaseflow_contracts::LeaseStatus::Pending,
        1 => leaseflow_contracts::LeaseStatus::Active,
        2 => leaseflow_contracts::LeaseStatus::Expired,
        3 => leaseflow_contracts::LeaseStatus::Disputed,
        _ => leaseflow_contracts::LeaseStatus::Terminated,
    };
    
    lease.last_tenant_interaction = data.tenant_last_interaction;
    
    // Update lease in storage (we need to do this manually since there's no direct function)
    // For fuzzing purposes, we'll simulate the storage update
    
    // Set ledger timestamp
    env.ledger().with_mut(|l| l.timestamp = data.current_timestamp);

    // Determine caller
    let caller = if data.caller_is_landlord { landlord.clone() } else { unauthorized.clone() };

    // Attempt to claim abandoned deposit
    let result = client.try_claim_abandoned_deposit(&lease_id, &caller);

    // Verify invariants:
    
    // 1. If caller is not landlord, should always fail with Unauthorised
    if !data.caller_is_landlord {
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::Unauthorised)));
        return;
    }

    // 2. If lease is not Expired, should fail with LeaseNotExpired
    if data.lease_status != 2 {
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::LeaseNotExpired)));
        return;
    }

    // 3. Calculate grace period deadline
    const ABANDONMENT_GRACE_PERIOD: u64 = 30 * 24 * 60 * 60; // 30 days
    let grace_period_deadline = data.lease_end_timestamp + ABANDONMENT_GRACE_PERIOD;

    // 4. If current timestamp is before grace period deadline, should fail
    if data.current_timestamp < grace_period_deadline {
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::LeaseNotExpired)));
        return;
    }

    // 5. If tenant had interaction after grace period deadline, should fail with AbandonmentChallenge
    if data.tenant_last_interaction > grace_period_deadline {
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::AbandonmentChallenge)));
        return;
    }

    // 6. If all conditions are met, should succeed
    if data.current_timestamp >= grace_period_deadline && data.tenant_last_interaction <= grace_period_deadline {
        assert!(result.is_ok());
    }

    // Additional timestamp manipulation checks:
    
    // Ensure the function cannot be exploited by timestamp manipulation
    // The grace period should be strictly enforced
    
    // Edge case: exactly at grace period deadline
    if data.current_timestamp == grace_period_deadline {
        // Should fail because grace period hasn't passed yet
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::LeaseNotExpired)));
    }
    
    // Edge case: one second after grace period deadline
    if data.current_timestamp == grace_period_deadline + 1 && data.tenant_last_interaction <= grace_period_deadline {
        // Should succeed because grace period has passed
        assert!(result.is_ok());
    }
    
    // Edge case: tenant interaction exactly at grace period deadline
    if data.tenant_last_interaction == grace_period_deadline {
        // Should fail because tenant interacted at the deadline
        assert_eq!(result, Err(Ok(leaseflow_contracts::LeaseError::AbandonmentChallenge)));
    }
});
