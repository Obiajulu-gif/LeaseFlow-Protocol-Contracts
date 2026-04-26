#[cfg(test)]
mod test {
    use soroban_sdk::{Address, Bytes, Env};
    use crate::{LeaseFlowContract, LeaseState, Error, Lease};

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

        fn get_lease(&self, lease_id: &u64) -> Lease {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_lease"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into().unwrap()
        }
    }
}
