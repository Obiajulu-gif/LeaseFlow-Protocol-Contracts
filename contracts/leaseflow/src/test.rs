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
        ) -> u64 {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("create_lease"),
                soroban_sdk::xdr::ScVal::try_from((
                    lessor, lessee, rent_amount, deposit_amount, 
                    start_date, end_date, max_grace_period, late_fee_rate, property_uri
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
    }
}
