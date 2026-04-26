#[cfg(test)]
mod test {
    use soroban_sdk::{Address, Bytes, Env};
    use crate::{LeaseFlowContract, LeaseState, Error, Lease, EscrowVault, ProtocolCreditRecord};

    #[test]
    fn test_lease_creation_and_states() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract
        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        let start_date = 1000;
        let end_date = 5000;
        let max_grace_period = 432000; // 5 days in seconds
        let late_fee_rate = 500; // 5% in basis points
        let property_uri = Bytes::from_slice(&env, b"property_uri");

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &max_grace_period,
            &late_fee_rate,
            &property_uri,
        );

        assert_eq!(lease_id, 1);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.lease_id, 1);
        assert_eq!(lease.state, LeaseState::Pending);
        assert_eq!(lease.max_grace_period, max_grace_period);
        assert_eq!(lease.late_fee_rate, late_fee_rate);
        assert!(!lease.arrears_processed);
    }

    #[test]
    fn test_grace_period_flow() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000, // 5 days
            &500,    // 5% late fee
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Verify active state
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::Active);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        let grace_lease = client.get_lease(&lease_id);
        assert_eq!(grace_lease.state, LeaseState::GracePeriod);
        assert!(grace_lease.dunning_start_timestamp.is_some());
        assert_eq!(grace_lease.outstanding_balance, 1000);
        assert_eq!(grace_lease.accumulated_late_fees, 50); // 5% of 1000

        // Recover during grace period
        client.process_rent_payment(&lease_id, &1050); // 1000 rent + 50 late fee

        let recovered_lease = client.get_lease(&lease_id);
        assert_eq!(recovered_lease.state, LeaseState::Active);
        assert_eq!(recovered_lease.outstanding_balance, 0);
        assert_eq!(recovered_lease.accumulated_late_fees, 0);
        assert!(recovered_lease.dunning_start_timestamp.is_none());
    }

    #[test]
    fn test_automated_arrears_deduction_basic() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        let late_fee_rate = 500; // 5%

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000, // 5 days
            &late_fee_rate,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Check initial escrow vault state
        let vault = client.get_escrow_vault();
        assert_eq!(vault.total_locked, deposit_amount);
        assert_eq!(vault.available_balance, deposit_amount);
        assert_eq!(vault.lessor_treasury, 0);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Simulate grace period expiry (this will auto-trigger arrears deduction)
        client.check_grace_period_expiry(&lease_id);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::EvictionPending);
        assert!(lease.arrears_processed);

        // Check escrow vault after deduction
        let vault_after = client.get_escrow_vault();
        let expected_deduction = rent_amount + (rent_amount * late_fee_rate as i64 / 10000);
        assert_eq!(vault_after.available_balance, deposit_amount - expected_deduction);
        assert_eq!(vault_after.lessor_treasury, expected_deduction);

        // Check credit record (should be none since deposit covered full arrears)
        let credit_record = client.get_credit_record(&lessee);
        assert!(credit_record.is_err()); // No residual debt
    }

    #[test]
    fn test_arrears_deduction_with_residual_debt() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 500; // Smaller deposit than total arrears
        let late_fee_rate = 500; // 5%

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &late_fee_rate,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Simulate grace period expiry (auto-triggers arrears deduction)
        client.check_grace_period_expiry(&lease_id);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::EvictionPending);
        assert!(lease.arrears_processed);

        // Check escrow vault - should be fully drained
        let vault_after = client.get_escrow_vault();
        assert_eq!(vault_after.available_balance, 0);
        assert_eq!(vault_after.lessor_treasury, deposit_amount);

        // Check credit record for residual debt
        let credit_record = client.get_credit_record(&lessee).unwrap();
        let total_arrears = rent_amount + (rent_amount * late_fee_rate as i64 / 10000);
        let expected_residual = total_arrears - deposit_amount;
        assert_eq!(credit_record.total_debt_amount, expected_residual);
        assert_eq!(credit_record.default_count, 1);
        assert!(credit_record.associated_lease_ids.contains(&lease_id));
    }

    #[test]
    fn test_manual_arrears_deduction() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Manually trigger grace period expiry
        client.check_grace_period_expiry(&lease_id);

        // Try to execute arrears deduction again (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err()); // Already processed

        // Verify state
        let lease = client.get_lease(&lease_id);
        assert!(lease.arrears_processed);
    }

    #[test]
    fn test_arrears_deduction_state_validation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        // Try to execute arrears deduction from Pending state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());

        // Activate lease
        client.activate_lease(&lease_id, &lessee);

        // Try from Active state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Try from GracePeriod state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_credit_record_accumulation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 300; // Small to ensure residual debt

        // Create first lease
        let lease_id1 = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri1"),
        );

        client.activate_lease(&lease_id1, &lessee);
        client.handle_rent_payment_failure(&lease_id1);
        client.check_grace_period_expiry(&lease_id1);

        // Create second lease for same lessee
        let lease_id2 = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &6000,
            &10000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri2"),
        );

        client.activate_lease(&lease_id2, &lessee);
        client.handle_rent_payment_failure(&lease_id2);
        client.check_grace_period_expiry(&lease_id2);

        // Check accumulated credit record
        let credit_record = client.get_credit_record(&lessee).unwrap();
        assert_eq!(credit_record.default_count, 2);
        assert!(credit_record.associated_lease_ids.contains(&lease_id1));
        assert!(credit_record.associated_lease_ids.contains(&lease_id2));
        assert!(credit_record.total_debt_amount > 0);
    }

    #[test]
    fn test_prorated_rent_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 3100; // Amount divisible by 31 days
        let deposit_amount = 2000;
        
        // Create lease that starts in the past (mid-cycle scenario)
        let past_start = env.ledger().timestamp() - 5 * 86400; // Started 5 days ago
        let end_date = past_start + 31 * 86400; // 31-day lease
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &past_start,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        let lease = client.get_lease(&lease_id);
        
        // Should have prorated initial rent (26 days remaining out of 31)
        // 3100 * (26/31) = 2600
        assert_eq!(lease.prorated_initial_rent, 2600);
        assert_eq!(lease.total_paid_rent, 0);
    }

    #[test]
    fn test_prorated_rent_future_start() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        // Create lease that starts in the future
        let future_start = env.ledger().timestamp() + 10 * 86400; // Starts in 10 days
        let end_date = future_start + 30 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &future_start,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        let lease = client.get_lease(&lease_id);
        
        // Should have full rent (no proration for future start)
        assert_eq!(lease.prorated_initial_rent, rent_amount);
    }

    #[test]
    fn test_lease_termination_with_refund() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 3100; // Divisible by 31 days
        let deposit_amount = 2000;
        
        let start_date = env.ledger().timestamp();
        let end_date = start_date + 31 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Pay rent for tracking
        client.process_rent_payment(&lease_id, &rent_amount);
        
        // Advance time by 10 days
        env.ledger().set_timestamp(start_date + 10 * 86400);
        
        // Terminate lease (should refund for 21 remaining days)
        let refund = client.terminate_lease(&lease_id, &lessor);
        
        // Expected refund: 3100 * (21/31) = 2100, minus 1 stroop = 2099
        assert_eq!(refund, 2099);
        
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::Closed);
    }

    #[test]
    fn test_lease_termination_security_penalty() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        let start_date = env.ledger().timestamp();
        let end_date = start_date + 30 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        client.process_rent_payment(&lease_id, &rent_amount);
        
        // Terminate immediately (within 24 hours) - should apply penalty
        let refund = client.terminate_lease(&lease_id, &lessor);
        
        // Should apply 10% penalty for rapid termination
        // Full refund would be ~1000, penalty would be ~100, so refund ~900
        assert!(refund < 1000);
        assert!(refund > 800); // Should be reasonable
    }

    #[test]
    fn test_lease_termination_unauthorized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Try to terminate with unauthorized address
        let result = client.try_terminate_lease(&lease_id, &unauthorized);
        assert!(result.is_err());
    }

    #[test]
    fn test_prorated_rent_tracking() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &None, // No fiat peg
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Make multiple payments
        client.process_rent_payment(&lease_id, &rent_amount);
        client.process_rent_payment(&lease_id, &rent_amount);
        
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.total_paid_rent, 2000); // Should track total payments
    }

    #[test]
    fn test_fiat_pegged_lease_creation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100, // $100 USD target
            asset_address: asset_address.clone(),
            oracle_address: oracle_address.clone(),
            staleness_threshold: 900, // 15 minutes
            volatility_threshold: 2000, // 20%
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000, // Base rent in XLM
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        let lease = client.get_lease(&lease_id);
        assert!(lease.fiat_peg_config.is_some());
        let config = lease.fiat_peg_config.unwrap();
        assert_eq!(config.target_usd_amount, 100);
        assert_eq!(config.asset_address, asset_address);
        assert_eq!(config.oracle_address, oracle_address);
    }

    #[test]
    fn test_fiat_pegged_rent_calculation_bull_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle for bull market (XLM price increases from $0.10 to $0.20)
        let mock_oracle = MockSep40Oracle::new(&env);
        mock_oracle.set_price(&asset_address, &200000000, &7); // $0.20 with 7 decimals
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Process fiat-pegged rent - should require less XLM due to higher price
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        let lease = client.get_lease(&lease_id);
        // At $0.20 per XLM, $100 USD = 500 XLM (100 / 0.20)
        assert_eq!(lease.total_paid_rent, 500);
    }

    #[test]
    fn test_fiat_pegged_rent_calculation_bear_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle for bear market (XLM price drops to $0.05)
        let mock_oracle = MockSep40Oracle::new(&env);
        mock_oracle.set_price(&asset_address, &50000000, &7); // $0.05 with 7 decimals
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Process fiat-pegged rent - should require more XLM due to lower price
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        let lease = client.get_lease(&lease_id);
        // At $0.05 per XLM, $100 USD = 2000 XLM (100 / 0.05)
        assert_eq!(lease.total_paid_rent, 2000);
    }

    #[test]
    fn test_oracle_staleness_protection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle with stale price (20 minutes old)
        let mock_oracle = MockSep40Oracle::new(&env);
        let stale_timestamp = env.ledger().timestamp() - 1200; // 20 minutes ago
        mock_oracle.set_price_with_timestamp(&asset_address, &100000000, &7, &stale_timestamp);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900, // 15 minutes
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Should fail due to stale oracle data
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::OracleDataStale);
    }

    #[test]
    fn test_volatility_circuit_breaker() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000, // 20%
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // First payment with normal price
        mock_oracle.set_price(&asset_address, &100000000, &7); // $0.10
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        // Advance time by 30 minutes
        env.ledger().set_timestamp(env.ledger().timestamp() + 1800);
        
        // Second payment with extreme price change (50% increase)
        mock_oracle.set_price(&asset_address, &150000000, &7); // $0.15 (+50%)
        
        // Should fail due to volatility circuit breaker
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::VolatilityCircuitBreaker);
    }

    #[test]
    fn test_12_month_lease_simulation_bull_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let start_time = env.ledger().timestamp();
        let end_time = start_time + 12 * 30 * 86400; // 12 months
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &start_time,
            &end_time,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Simulate 12 months of bull market (price increasing from $0.10 to $0.50)
        let mut total_paid = 0;
        for month in 0..12 {
            let price = 100000000 + (month as i128 * 33333333); // Linear increase
            mock_oracle.set_price(&asset_address, &price, &7);
            
            env.ledger().set_timestamp(start_time + (month + 1) * 30 * 86400);
            client.process_fiat_pegged_rent_payment(&lease_id);
            
            let lease = client.get_lease(&lease_id);
            total_paid = lease.total_paid_rent;
        }
        
        // In bull market, total XLM paid should decrease over time
        // Early months: ~1000 XLM, Later months: ~200 XLM
        assert!(total_paid < 12000); // Should be significantly less than fixed 12000 XLM
        assert!(total_paid > 2400);  // But still reasonable amount
    }

    #[test]
    fn test_12_month_lease_simulation_bear_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let start_time = env.ledger().timestamp();
        let end_time = start_time + 12 * 30 * 86400; // 12 months
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &start_time,
            &end_time,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Simulate 12 months of bear market (price decreasing from $0.10 to $0.02)
        let mut total_paid = 0;
        for month in 0..12 {
            let price = 100000000 - (month as i128 * 6666666); // Linear decrease
            mock_oracle.set_price(&asset_address, &price.max(20000000), &7);
            
            env.ledger().set_timestamp(start_time + (month + 1) * 30 * 86400);
            client.process_fiat_pegged_rent_payment(&lease_id);
            
            let lease = client.get_lease(&lease_id);
            total_paid = lease.total_paid_rent;
        }
        
        // In bear market, total XLM paid should increase over time
        // Early months: ~1000 XLM, Later months: ~5000 XLM
        assert!(total_paid > 12000); // Should be significantly more than fixed 12000 XLM
        assert!(total_paid < 60000); // But still reasonable
    }

    #[test]
    fn test_flash_loan_attack_protection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000, // 20% threshold
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // First payment with normal price
        mock_oracle.set_price(&asset_address, &100000000, &7); // $0.10
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        // Simulate flash loan attack - extreme price manipulation in same block
        mock_oracle.set_price(&asset_address, &50000000, &7); // 50% drop
        
        // Should fail due to volatility circuit breaker protecting against flash loan attacks
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::VolatilityCircuitBreaker);
    }

    // Mock SEP-40 Oracle for testing
    struct MockSep40Oracle {
        env: Env,
        address: Address,
        prices: Map<Address, (i128, u32, u64)>, // (price, decimals, timestamp)
    }
    
    impl MockSep40Oracle {
        fn new(env: &Env) -> Self {
            let address = Address::generate(env);
            Self {
                env: env.clone(),
                address,
                prices: Map::new(env),
            }
        }
        
        fn set_price(&self, asset: &Address, price: &i128, decimals: &u32) {
            self.prices.set(asset, (*price, *decimals, self.env.ledger().timestamp()));
        }
        
        fn set_price_with_timestamp(&self, asset: &Address, price: &i128, decimals: &u32, timestamp: &u64) {
            self.prices.set(asset, (*price, *decimals, *timestamp));
        }
    }

    struct LeaseFlowContractClient<'a> {
        env: &'a Env,
        contract_id: &'a soroban_sdk::Address,
    }

    impl<'a> LeaseFlowContractClient<'a> {
        fn new(env: &'a Env, contract_id: &'a soroban_sdk::Address) -> Self {
            Self { env, contract_id }
        }

        fn initialize(&self) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("initialize"),
                soroban_sdk::xdr::ScVal::Void,
            );
        }

        fn initialize_with_oracle(&self, oracle_address: &Address) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("initialize"),
                soroban_sdk::xdr::ScVal::try_from(oracle_address).unwrap(),
            );
        }

        fn create_lease(
            &self,
            lessor: &Address,
            lessee: &Address,
            rent_amount: &i64,
            deposit_amount: &i64,
            start_date: &u64,
            end_date: &u64,
            max_grace_period: &u64,
            late_fee_rate: &u32,
            property_uri: &Bytes,
            fiat_peg_config: &Option<FiatPegConfig>,
        ) -> u64 {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("create_lease"),
                soroban_sdk::xdr::ScVal::try_from((
                    lessor, lessee, rent_amount, deposit_amount, 
                    start_date, end_date, max_grace_period, late_fee_rate, property_uri, fiat_peg_config
                )).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn activate_lease(&self, lease_id: &u64, lessee: &Address) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("activate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, lessee)).unwrap(),
            );
        }

        fn process_rent_payment(&self, lease_id: &u64, amount: &i64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, amount)).unwrap(),
            );
        }

        fn handle_rent_payment_failure(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("handle_rent_payment_failure"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn check_grace_period_expiry(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("check_grace_period_expiry"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn execute_arrears_deduction(&self, lease_id: &u64) -> Result<(), Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("execute_arrears_deduction"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into()
        }

        fn get_lease(&self, lease_id: &u64) -> Lease {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_lease"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn get_credit_record(&self, lessee: &Address) -> Result<ProtocolCreditRecord, Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_credit_record"),
                soroban_sdk::xdr::ScVal::try_from(lessee).unwrap(),
            );
            result.try_into()
        }

        fn get_escrow_vault(&self) -> EscrowVault {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_escrow_vault"),
                soroban_sdk::xdr::ScVal::Void,
            );
            result.try_into().unwrap()
        }

        fn terminate_lease(&self, lease_id: &u64, caller: &Address) -> i64 {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("terminate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, caller)).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn try_terminate_lease(&self, lease_id: &u64, caller: &Address) -> Result<i64, Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("terminate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, caller)).unwrap(),
            );
            result.try_into()
        }

        fn process_fiat_pegged_rent_payment(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_fiat_pegged_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn try_process_fiat_pegged_rent_payment(&self, lease_id: &u64) -> Result<(), Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_fiat_pegged_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into()
        }
    }
}
