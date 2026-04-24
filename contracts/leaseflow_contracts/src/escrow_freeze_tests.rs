use soroban_sdk::{Address, Env, Symbol, testutils::{Address as TestAddress, Ledger as TestLedger}, BytesN};
use crate::escrow_vault::{EscrowVault, ContractError, EscrowEntry};
use crate::continuous_billing_module::{ContinuousBillingModule, ActiveLease, BillingConfig};

#[test]
fn test_escrow_freeze_circuit_breaker() {
    let env = Env::default();
    
    // Setup addresses
    let admin = Address::generate(&env);
    let dao_security_council = Address::generate(&env);
    let attacker = Address::generate(&env);
    let victim_depositor = Address::generate(&env);
    let victim_beneficiary = Address::generate(&env);
    let oracle = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    // Initialize contracts
    EscrowVault::initialize(&env, admin.clone(), dao_security_council.clone()).unwrap();
    
    // Mock token setup (simplified)
    let initial_balance = 1000000000i128; // 1000 tokens
    
    // Test 1: Normal operation before freeze
    println!("Test 1: Normal operation before freeze");
    
    // Create escrow deposit
    let escrow_id = EscrowVault::initialize_deposit(
        &env,
        victim_depositor.clone(),
        victim_beneficiary.clone(),
        token_address.clone(),
        100000000i128, // 100 tokens
        String::from_str(&env, "security_deposit"),
        86400, // 24 hours lock
        Some(12345), // lease_id
    ).unwrap();
    
    println!("✓ Escrow deposit created successfully: {}", escrow_id);
    
    // Verify escrow exists
    let escrow = EscrowVault::get_escrow(&env, escrow_id).unwrap();
    assert_eq!(escrow.amount, 100000000i128);
    assert_eq!(escrow.status, String::from_str(&env, "pending"));
    
    // Test 2: Activate emergency freeze
    println!("\nTest 2: Activate emergency freeze");
    
    // DAO Security Council activates freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        true,
        String::from_str(&env, "Critical zero-day exploit detected - immediate freeze required"),
    ).unwrap();
    
    // Verify freeze is active
    assert!(EscrowVault::is_escrow_frozen(&env));
    println!("✓ Emergency freeze activated successfully");
    
    // Test 3: Attempt all escrow operations during freeze (should all fail)
    println!("\nTest 3: Exploit simulation - all escrow operations should fail during freeze");
    
    // 3.1: Attempt to initialize new deposit (should fail)
    let result = EscrowVault::initialize_deposit(
        &env,
        attacker.clone(),
        victim_beneficiary.clone(),
        token_address.clone(),
        50000000i128,
        String::from_str(&env, "malicious_deposit"),
        86400,
        Some(99999),
    );
    
    assert_eq!(result.err(), Some(ContractError::EscrowFrozen));
    println!("✓ Deposit initialization blocked: EscrowFrozen");
    
    // 3.2: Attempt Oracle slashing (should fail)
    let result = EscrowVault::execute_oracle_slash(
        &env,
        oracle.clone(),
        escrow_id,
        25000000i128,
        attacker.clone(),
    );
    
    assert_eq!(result.err(), Some(ContractError::EscrowFrozen));
    println!("✓ Oracle slashing blocked: EscrowFrozen");
    
    // 3.3: Attempt mutual release (should fail)
    let result = EscrowVault::execute_mutual_release(
        &env,
        victim_depositor.clone(),
        escrow_id,
        30000000i128,
        victim_beneficiary.clone(),
    );
    
    assert_eq!(result.err(), Some(ContractError::EscrowFrozen));
    println!("✓ Mutual release blocked: EscrowFrozen");
    
    // 3.4: Attempt arrears deduction (should fail)
    let result = EscrowVault::deduct_arrears(
        &env,
        oracle.clone(),
        escrow_id,
        20000000i128,
        attacker.clone(),
    );
    
    assert_eq!(result.err(), Some(ContractError::EscrowFrozen));
    println!("✓ Arrears deduction blocked: EscrowFrozen");
    
    println!("✓ All exploit attempts blocked successfully");
    
    // Test 4: Verify continuous billing still works during freeze
    println!("\nTest 4: Continuous billing module remains operational during freeze");
    
    // Initialize continuous billing module
    ContinuousBillingModule::initialize(&env, admin.clone()).unwrap();
    
    // Register lease for billing
    ContinuousBillingModule::register_lease_billing(
        &env,
        12345u64,
        victim_depositor.clone(),
        victim_beneficiary.clone(),
        Address::generate(&env),
        10000000i128, // 0.1 tokens rent
        token_address.clone(),
        env.ledger().timestamp(),
        env.ledger().timestamp() + 2592000, // 30 days
        86400, // daily billing
    ).unwrap();
    
    println!("✓ Lease billing registration works during freeze");
    
    // Process billing cycle
    let cycle_id = ContinuousBillingModule::process_billing_cycle(
        &env,
        12345u64,
        admin.clone(),
    ).unwrap();
    
    println!("✓ Billing cycle processing works during freeze: cycle {}", cycle_id);
    
    // Test 5: Lift freeze and verify operations resume
    println!("\nTest 5: Lift freeze and verify operations resume");
    
    // Get freeze timestamp
    let freeze_timestamp = EscrowVault::get_freeze_timestamp(&env).unwrap();
    println!("Freeze was activated at timestamp: {}", freeze_timestamp);
    
    // Simulate time passing
    env.ledger().set_timestamp(env.ledger().timestamp() + 3600); // 1 hour later
    
    // DAO Security Council lifts freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        false,
        String::from_str(&env, "Exploit patched - normal operations resumed"),
    ).unwrap();
    
    // Verify freeze is lifted
    assert!(!EscrowVault::is_escrow_frozen(&env));
    println!("✓ Emergency freeze lifted successfully");
    
    // Test 6: Verify operations work after freeze lift
    println!("\nTest 6: Verify operations work after freeze lift");
    
    // 6.1: Initialize new deposit (should work)
    let new_escrow_id = EscrowVault::initialize_deposit(
        &env,
        victim_depositor.clone(),
        victim_beneficiary.clone(),
        token_address.clone(),
        75000000i128,
        String::from_str(&env, "post_freeze_deposit"),
        86400,
        Some(54321),
    ).unwrap();
    
    println!("✓ Deposit initialization works after freeze lift: {}", new_escrow_id);
    
    // 6.2: Execute mutual release on original escrow (should work)
    EscrowVault::execute_mutual_release(
        &env,
        victim_depositor.clone(),
        escrow_id,
        40000000i128,
        victim_beneficiary.clone(),
    ).unwrap();
    
    println!("✓ Mutual release works after freeze lift");
    
    // Test 7: Mathematical resolution for leases expired during freeze
    println!("\nTest 7: Mathematical resolution for leases expired during freeze");
    
    // Simulate lease expiration during freeze period
    let expired_lease_id = 99999u64;
    ContinuousBillingModule::register_lease_billing(
        &env,
        expired_lease_id,
        victim_depositor.clone(),
        victim_beneficiary.clone(),
        Address::generate(&env),
        5000000i128,
        token_address.clone(),
        env.ledger().timestamp() - 86400, // started yesterday
        env.ledger().timestamp() - 3600, // expired 1 hour ago
        43200, // 12 hour billing
    ).unwrap();
    
    // Check lease expiration
    let is_expired = ContinuousBillingModule::check_lease_expiration(&env, expired_lease_id).unwrap();
    assert!(is_expired);
    println!("✓ Lease expiration detection works correctly");
    
    // Calculate final settlement
    let (remaining_rent, current_arrears) = ContinuousBillingModule::calculate_final_settlement(&env, expired_lease_id).unwrap();
    println!("✓ Final settlement calculated: remaining_rent={}, current_arrears={}", remaining_rent, current_arrears);
    
    println!("\n=== ALL TESTS PASSED ===");
    println!("✓ Global Escrow Freeze Circuit Breaker is working correctly");
    println!("✓ All escrow operations are blocked during freeze");
    println!("✓ Continuous billing remains operational during freeze");
    println!("✓ Mathematical resolution works for expired leases");
    println!("✓ Operations resume correctly after freeze lift");
}

#[test]
fn test_freeze_access_control() {
    let env = Env::default();
    
    let admin = Address::generate(&env);
    let dao_security_council = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    
    // Initialize contract
    EscrowVault::initialize(&env, admin.clone(), dao_security_council.clone()).unwrap();
    
    // Test 1: Unauthorized user cannot activate freeze
    let result = EscrowVault::toggle_escrow_freeze(
        &env,
        unauthorized_user.clone(),
        true,
        String::from_str(&env, "Unauthorized freeze attempt"),
    );
    
    assert_eq!(result.err(), Some(ContractError::Unauthorized));
    println!("✓ Unauthorized freeze attempt blocked");
    
    // Test 2: DAO Security Council can activate freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        true,
        String::from_str(&env, "Authorized freeze activation"),
    ).unwrap();
    
    assert!(EscrowVault::is_escrow_frozen(&env));
    println!("✓ DAO Security Council successfully activated freeze");
    
    // Test 3: Unauthorized user cannot lift freeze
    let result = EscrowVault::toggle_escrow_freeze(
        &env,
        unauthorized_user.clone(),
        false,
        String::from_str(&env, "Unauthorized lift attempt"),
    );
    
    assert_eq!(result.err(), Some(ContractError::Unauthorized));
    assert!(EscrowVault::is_escrow_frozen(&env));
    println!("✓ Unauthorized lift attempt blocked");
    
    // Test 4: DAO Security Council can lift freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        false,
        String::from_str(&env, "Authorized freeze lift"),
    ).unwrap();
    
    assert!(!EscrowVault::is_escrow_frozen(&env));
    println!("✓ DAO Security Council successfully lifted freeze");
}

#[test]
fn test_freeze_persistence_and_events() {
    let env = Env::default();
    
    let admin = Address::generate(&env);
    let dao_security_council = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    // Initialize contract
    EscrowVault::initialize(&env, admin.clone(), dao_security_council.clone()).unwrap();
    
    // Create initial escrow
    let escrow_id = EscrowVault::initialize_deposit(
        &env,
        depositor.clone(),
        beneficiary.clone(),
        token_address.clone(),
        100000000i128,
        String::from_str(&env, "test_deposit"),
        86400,
        Some(11111),
    ).unwrap();
    
    // Activate freeze and capture events
    let freeze_start_time = env.ledger().timestamp();
    
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        true,
        String::from_str(&env, "Test freeze activation"),
    ).unwrap();
    
    // Verify freeze timestamp is set
    let freeze_timestamp = EscrowVault::get_freeze_timestamp(&env).unwrap();
    assert_eq!(freeze_timestamp, freeze_start_time);
    println!("✓ Freeze timestamp correctly recorded");
    
    // Simulate time passing
    env.ledger().set_timestamp(freeze_start_time + 7200); // 2 hours later
    
    // Lift freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        false,
        String::from_str(&env, "Test freeze lift"),
    ).unwrap();
    
    // Verify freeze timestamp is cleared
    assert_eq!(EscrowVault::get_freeze_timestamp(&env), None);
    println!("✓ Freeze timestamp correctly cleared");
    
    // Verify operations work after freeze
    let new_escrow_id = EscrowVault::initialize_deposit(
        &env,
        depositor.clone(),
        beneficiary.clone(),
        token_address.clone(),
        50000000i128,
        String::from_str(&env, "post_freeze_test"),
        86400,
        Some(22222),
    ).unwrap();
    
    println!("✓ Operations work correctly after freeze lift");
    
    // Test freeze state persistence across multiple operations
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        true,
        String::from_str(&env, "Persistence test freeze"),
    ).unwrap();
    
    // Multiple attempts should all fail consistently
    for i in 1..=5 {
        let result = EscrowVault::initialize_deposit(
            &env,
            depositor.clone(),
            beneficiary.clone(),
            token_address.clone(),
            1000000i128 * i as i128,
            String::from_str(&env, &format!("attempt_{}", i)),
            86400,
            Some(33333 + i),
        );
        assert_eq!(result.err(), Some(ContractError::EscrowFrozen));
    }
    
    println!("✓ Freeze state persists consistently across multiple operations");
}

#[test]
fn test_continuous_billing_during_freeze() {
    let env = Env::default();
    
    let admin = Address::generate(&env);
    let dao_security_council = Address::generate(&env);
    let lessor = Address::generate(&env);
    let lessee = Address::generate(&env);
    let asset_address = Address::generate(&env);
    let token_address = Address::generate(&env);
    
    // Initialize both contracts
    EscrowVault::initialize(&env, admin.clone(), dao_security_council.clone()).unwrap();
    ContinuousBillingModule::initialize(&env, admin.clone()).unwrap();
    
    // Register lease for billing
    let lease_id = 12345u64;
    ContinuousBillingModule::register_lease_billing(
        &env,
        lease_id,
        lessor.clone(),
        lessee.clone(),
        asset_address.clone(),
        10000000i128, // 0.1 tokens rent
        token_address.clone(),
        env.ledger().timestamp(),
        env.ledger().timestamp() + 2592000, // 30 days
        86400, // daily billing
    ).unwrap();
    
    // Activate escrow freeze
    EscrowVault::toggle_escrow_freeze(
        &env,
        dao_security_council.clone(),
        true,
        String::from_str(&env, "Test freeze during billing"),
    ).unwrap();
    
    // Verify escrow is frozen
    assert!(EscrowVault::is_escrow_frozen(&env));
    
    // Test continuous billing operations during freeze
    println!("Testing continuous billing during escrow freeze...");
    
    // Process billing cycle (should work)
    let cycle_id = ContinuousBillingModule::process_billing_cycle(
        &env,
        lease_id,
        admin.clone(),
    ).unwrap();
    
    println!("✓ Billing cycle processed during freeze: {}", cycle_id);
    
    // Process payment (should work)
    ContinuousBillingModule::process_payment(
        &env,
        lessee.clone(),
        cycle_id,
        10000000i128,
    ).unwrap();
    
    println!("✓ Payment processed during freeze");
    
    // Register new lease (should work)
    let new_lease_id = 54321u64;
    ContinuousBillingModule::register_lease_billing(
        &env,
        new_lease_id,
        lessor.clone(),
        lessee.clone(),
        asset_address.clone(),
        15000000i128,
        token_address.clone(),
        env.ledger().timestamp(),
        env.ledger().timestamp() + 5184000, // 60 days
        86400,
    ).unwrap();
    
    println!("✓ New lease registered during freeze");
    
    // Get billing configuration (should work)
    let config = ContinuousBillingModule::get_billing_config(&env);
    assert_eq!(config.grace_period, 86400);
    println!("✓ Billing configuration accessible during freeze");
    
    // Check lease expiration (should work)
    let is_expired = ContinuousBillingModule::check_lease_expiration(&env, lease_id).unwrap();
    assert!(!is_expired);
    println!("✓ Lease expiration check works during freeze");
    
    println!("✓ All continuous billing operations work correctly during escrow freeze");
}
