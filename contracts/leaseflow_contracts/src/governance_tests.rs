#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};

// ============================================================================
// Issue #117: Multi-Sig Veto Tests
// ============================================================================

#[test]
fn test_initialize_security_council() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let council_member_1 = Address::generate(&env);
    let council_member_2 = Address::generate(&env);
    let council_member_3 = Address::generate(&env);

    env.set_contract_address(admin.clone());

    // Initialize admin
    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Create council members
    let mut members = soroban_sdk::Vec::new(&env);
    members.push_back(SecurityCouncilMember {
        address: council_member_1.clone(),
        voting_power: 4000, // 40%
        active: true,
    });
    members.push_back(SecurityCouncilMember {
        address: council_member_2.clone(),
        voting_power: 3500, // 35%
        active: true,
    });
    members.push_back(SecurityCouncilMember {
        address: council_member_3.clone(),
        voting_power: 2500, // 25%
        active: true,
    });

    // Initialize council
    let result = LeaseContract::initialize_security_council(
        env.clone(),
        admin.clone(),
        members,
        6000, // 60% threshold
    );
    assert!(result.is_ok());

    // Verify council was created
    let council = env
        .storage()
        .instance()
        .get::<DataKey, SecurityCouncil>(&DataKey::SecurityCouncil)
        .unwrap();
    assert_eq!(council.veto_threshold_bps, 6000);
    assert_eq!(council.total_voting_power, 10000);
}

#[test]
fn test_massive_slash_triggers_veto() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);
    let oracle_pubkey = BytesN::from_array(&env, &[1; 32]);

    env.set_contract_address(admin.clone());

    // Setup
    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
    LeaseContract::whitelist_oracle(env.clone(), admin.clone(), oracle_pubkey.clone()).unwrap();

    // Initialize security council
    let mut members = soroban_sdk::Vec::new(&env);
    members.push_back(SecurityCouncilMember {
        address: Address::generate(&env),
        voting_power: 10000,
        active: true,
    });
    LeaseContract::initialize_security_council(
        env.clone(),
        admin.clone(),
        members,
        6000,
    )
    .unwrap();

    // Create lease with massive deposit
    let lease_params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000_0000000,
        deposit_amount: 0,
        security_deposit: 20000_0000000, // 20,000 tokens (above threshold)
        start_date: env.ledger().timestamp(),
        end_date: env.ledger().timestamp() + 365 * 24 * 60 * 60,
        property_uri: String::from_str(&env, "ipfs://test"),
        payment_token: Address::generate(&env),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1000,
        grace_period_end: 0,
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

    let lease_id = 1u64;
    LeaseContract::create_lease_instance(
        env.clone(),
        lease_id,
        landlord.clone(),
        lease_params,
    )
    .unwrap();

    // Terminate lease
    let mut lease = LeaseContract::get_lease_instance(env.clone(), lease_id).unwrap();
    lease.status = LeaseStatus::Terminated;
    // Save updated lease (would need helper function in real implementation)

    // Create oracle payload for severe damage (100% slash)
    let payload = OraclePayload {
        lease_id,
        oracle_pubkey,
        damage_severity: DamageSeverity::Catastrophic,
        nonce: 1,
        timestamp: env.ledger().timestamp(),
        signature: BytesN::from_array(&env, &[0; 64]),
    };

    // Execute deposit slash - should trigger veto
    let result = LeaseContract::execute_deposit_slash(env.clone(), payload);
    assert_eq!(result, Err(LeaseError::PendingVeto));

    // Verify pending slash was created
    let pending_slash = env
        .storage()
        .instance()
        .get::<DataKey, PendingSlashVeto>(&DataKey::PendingSlash(lease_id))
        .unwrap();
    assert_eq!(pending_slash.lease_id, lease_id);
    assert!(!pending_slash.executed);
    assert!(!pending_slash.vetoed);
}

#[test]
fn test_veto_vote_execution() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let council_member = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Initialize council
    let mut members = soroban_sdk::Vec::new(&env);
    members.push_back(SecurityCouncilMember {
        address: council_member.clone(),
        voting_power: 10000,
        active: true,
    });
    LeaseContract::initialize_security_council(
        env.clone(),
        admin.clone(),
        members,
        6000,
    )
    .unwrap();

    // Create pending slash
    let current_time = env.ledger().timestamp();
    let pending_slash = PendingSlashVeto {
        lease_id: 1,
        slash_amount: 15000_0000000,
        tenant_refund: 5000_0000000,
        landlord_payout: 15000_0000000,
        oracle_payload: OraclePayload {
            lease_id: 1,
            oracle_pubkey: BytesN::from_array(&env, &[1; 32]),
            damage_severity: DamageSeverity::Severe,
            nonce: 1,
            timestamp: current_time,
            signature: BytesN::from_array(&env, &[0; 64]),
        },
        proposed_at: current_time,
        timelock_end: current_time + VETO_TIMELOCK_PERIOD,
        veto_votes_for: 0,
        veto_votes_against: 0,
        executed: false,
        vetoed: false,
    };

    env.storage()
        .instance()
        .set(&DataKey::PendingSlash(1), &pending_slash);

    // Vote for veto
    let result =
        LeaseContract::veto_slash_vote(env.clone(), council_member.clone(), 1, true);
    assert!(result.is_ok());

    // Verify vote was recorded
    let vote_key = DataKey::VetoVote(1, council_member.clone());
    let voted = env.storage().instance().get::<DataKey, bool>(&vote_key);
    assert!(voted.is_some());
}

// ============================================================================
// Issue #118: Dynamic Protocol Fee Tests
// ============================================================================

#[test]
fn test_initialize_protocol_fee_config() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    let config = ProtocolFeeConfig {
        current_fee_bps: 100, // 1%
        max_fee_bps: 3000,    // 30%
        min_fee_bps: 0,
        max_increase_bps: 500, // 5%
        update_timelock: 7 * 24 * 60 * 60,
    };

    let result = LeaseContract::initialize_protocol_fee_config(
        env.clone(),
        admin.clone(),
        config,
    );
    assert!(result.is_ok());

    // Verify config
    let stored_config = LeaseContract::get_protocol_fee_config(env.clone()).unwrap();
    assert_eq!(stored_config.current_fee_bps, 100);
    assert_eq!(stored_config.max_fee_bps, 3000);
}

#[test]
fn test_propose_fee_update_within_limits() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Initialize fee config
    let config = ProtocolFeeConfig {
        current_fee_bps: 100,
        max_fee_bps: 3000,
        min_fee_bps: 0,
        max_increase_bps: 500,
        update_timelock: 7 * 24 * 60 * 60,
    };
    LeaseContract::initialize_protocol_fee_config(
        env.clone(),
        admin.clone(),
        config,
    )
    .unwrap();

    // Propose fee update (within limits)
    let result = LeaseContract::propose_fee_update(env.clone(), proposer.clone(), 600);
    assert!(result.is_ok());

    // Verify proposal
    let pending = env
        .storage()
        .instance()
        .get::<DataKey, PendingFeeUpdate>(&DataKey::PendingFeeUpdate)
        .unwrap();
    assert_eq!(pending.proposed_fee_bps, 600);
    assert_eq!(pending.proposed_by, proposer);
}

#[test]
fn test_propose_fee_update_exceeds_max_increase() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    let config = ProtocolFeeConfig {
        current_fee_bps: 100,
        max_fee_bps: 3000,
        min_fee_bps: 0,
        max_increase_bps: 500,
        update_timelock: 7 * 24 * 60 * 60,
    };
    LeaseContract::initialize_protocol_fee_config(
        env.clone(),
        admin.clone(),
        config,
    )
    .unwrap();

    // Propose fee update exceeding max increase (100 + 500 = 600 max allowed)
    let result = LeaseContract::propose_fee_update(env.clone(), proposer.clone(), 700);
    assert_eq!(result, Err(LeaseError::InvalidParameters));
}

#[test]
fn test_execute_fee_update_after_timelock() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let proposer = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    let config = ProtocolFeeConfig {
        current_fee_bps: 100,
        max_fee_bps: 3000,
        min_fee_bps: 0,
        max_increase_bps: 500,
        update_timelock: 7 * 24 * 60 * 60,
    };
    LeaseContract::initialize_protocol_fee_config(
        env.clone(),
        admin.clone(),
        config.clone(),
    )
    .unwrap();

    // Propose fee update
    LeaseContract::propose_fee_update(env.clone(), proposer.clone(), 600).unwrap();

    // Fast forward past timelock
    let future_time = env.ledger().timestamp() + config.update_timelock + 1;
    env.ledger().set_timestamp(future_time);

    // Vote for the proposal
    LeaseContract::vote_on_fee_update(env.clone(), proposer.clone(), true).unwrap();

    // Execute fee update
    let result = LeaseContract::execute_fee_update(env.clone());
    assert!(result.is_ok());

    // Verify fee was updated
    let updated_config = LeaseContract::get_protocol_fee_config(env.clone()).unwrap();
    assert_eq!(updated_config.current_fee_bps, 600);
}

// ============================================================================
// Issue #119: Quadratic Voting Tests
// ============================================================================

#[test]
fn test_create_governance_round() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Create allocation options
    let mut options = soroban_sdk::Vec::new(&env);
    options.push_back(AllocationOption {
        option_id: 1,
        description: String::from_str(&env, "Option A"),
        total_quadratic_votes: 0,
        recipient_address: Address::generate(&env),
    });
    options.push_back(AllocationOption {
        option_id: 2,
        description: String::from_str(&env, "Option B"),
        total_quadratic_votes: 0,
        recipient_address: Address::generate(&env),
    });

    let result = LeaseContract::create_governance_round(
        env.clone(),
        admin.clone(),
        1,
        100000_0000000,
        options,
    );
    assert!(result.is_ok());

    // Verify round created
    let round = env
        .storage()
        .instance()
        .get::<DataKey, GovernanceRound>(&DataKey::GovernanceRound(1))
        .unwrap();
    assert_eq!(round.round_id, 1);
    assert!(round.active);
    assert_eq!(round.total_treasury_yield, 100000_0000000);
}

#[test]
fn test_quadratic_voting_power_calculation() {
    // Test integer sqrt function
    assert_eq!(LeaseContract::integer_sqrt(100), 10);
    assert_eq!(LeaseContract::integer_sqrt(10000), 100);
    assert_eq!(LeaseContract::integer_sqrt(1), 1);
    assert_eq!(LeaseContract::integer_sqrt(0), 0);
}

#[test]
fn test_cast_treasury_vote() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Create governance round
    let mut options = soroban_sdk::Vec::new(&env);
    options.push_back(AllocationOption {
        option_id: 1,
        description: String::from_str(&env, "Option A"),
        total_quadratic_votes: 0,
        recipient_address: recipient.clone(),
    });

    LeaseContract::create_governance_round(
        env.clone(),
        admin.clone(),
        1,
        100000_0000000,
        options,
    )
    .unwrap();

    // Cast vote with 10000 tokens (voting power = sqrt(10000) = 100)
    let result = LeaseContract::cast_treasury_vote(
        env.clone(),
        voter.clone(),
        1,
        1,
        10000_0000000,
    );
    assert!(result.is_ok());

    // Verify vote recorded
    let vote = env
        .storage()
        .instance()
        .get::<DataKey, TreasuryVote>(&DataKey::TreasuryVote(1, voter.clone()))
        .unwrap();
    assert_eq!(vote.voter, voter);
    assert_eq!(vote.option_id, 1);
    assert_eq!(vote.tokens_committed, 10000_0000000);
    assert_eq!(vote.voting_power, 100000000); // sqrt(10000 * 10^8) scaled
}

#[test]
fn test_finalize_governance_round() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Create governance round
    let mut options = soroban_sdk::Vec::new(&env);
    options.push_back(AllocationOption {
        option_id: 1,
        description: String::from_str(&env, "Option A"),
        total_quadratic_votes: 0,
        recipient_address: recipient_a.clone(),
    });
    options.push_back(AllocationOption {
        option_id: 2,
        description: String::from_str(&env, "Option B"),
        total_quadratic_votes: 0,
        recipient_address: recipient_b.clone(),
    });

    let round_start = env.ledger().timestamp();
    LeaseContract::create_governance_round(
        env.clone(),
        admin.clone(),
        1,
        100000_0000000,
        options,
    )
    .unwrap();

    // Cast votes
    LeaseContract::cast_treasury_vote(
        env.clone(),
        voter1.clone(),
        1,
        1,
        10000_0000000,
    )
    .unwrap();

    LeaseContract::cast_treasury_vote(
        env.clone(),
        voter2.clone(),
        1,
        2,
        5000_0000000,
    )
    .unwrap();

    // Fast forward past round end
    let future_time = round_start + GOVERNANCE_ROUND_DURATION + 1;
    env.ledger().set_timestamp(future_time);

    // Finalize round
    let result = LeaseContract::finalize_governance_round(env.clone(), 1);
    assert!(result.is_ok());

    let final_options = result.unwrap();
    assert_eq!(final_options.len(), 2);

    // Verify round is deactivated
    let round = env
        .storage()
        .instance()
        .get::<DataKey, GovernanceRound>(&DataKey::GovernanceRound(1))
        .unwrap();
    assert!(!round.active);
}

// ============================================================================
// Issue #124: Optimized get_active_leases Tests
// ============================================================================

#[test]
fn test_get_active_leases_empty() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Get active leases when none exist
    let active_leases = LeaseContract::get_active_leases(env.clone());
    assert_eq!(active_leases.len(), 0);
}

#[test]
fn test_get_active_leases_returns_active_only() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let landlord = Address::generate(&env);
    let tenant = Address::generate(&env);

    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();
    LeaseContract::add_allowed_asset(env.clone(), admin.clone(), Address::generate(&env)).unwrap();

    // Create active lease
    let lease_params = CreateLeaseParams {
        tenant: tenant.clone(),
        rent_amount: 1000_0000000,
        deposit_amount: 500_0000000,
        security_deposit: 1000_0000000,
        start_date: env.ledger().timestamp(),
        end_date: env.ledger().timestamp() + 365 * 24 * 60 * 60,
        property_uri: String::from_str(&env, "ipfs://test1"),
        payment_token: Address::generate(&env),
        arbitrators: soroban_sdk::Vec::new(&env),
        rent_per_sec: 1000,
        grace_period_end: 0,
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

    LeaseContract::create_lease_instance(
        env.clone(),
        1,
        landlord.clone(),
        lease_params.clone(),
    )
    .unwrap();

    // Get active leases
    let active_leases = LeaseContract::get_active_leases(env.clone());
    assert_eq!(active_leases.len(), 1);

    let summary = active_leases.get(0).unwrap();
    assert_eq!(summary.lease_id, 1);
    assert_eq!(summary.landlord, landlord);
    assert_eq!(summary.tenant, tenant);
    assert!(summary.active);
}

#[test]
fn test_active_leases_index_maintenance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    env.set_contract_address(admin.clone());

    LeaseContract::set_admin(env.clone(), admin.clone()).unwrap();

    // Add leases to index
    LeaseContract::add_to_active_leases_index(&env, 1).unwrap();
    LeaseContract::add_to_active_leases_index(&env, 2).unwrap();
    LeaseContract::add_to_active_leases_index(&env, 3).unwrap();

    // Verify index
    let index = env
        .storage()
        .instance()
        .get::<DataKey, soroban_sdk::Vec<u64>>(&DataKey::ActiveLeasesIndex)
        .unwrap();
    assert_eq!(index.len(), 3);

    // Remove from index
    LeaseContract::remove_from_active_leases_index(&env, 2).unwrap();

    let index = env
        .storage()
        .instance()
        .get::<DataKey, soroban_sdk::Vec<u64>>(&DataKey::ActiveLeasesIndex)
        .unwrap();
    assert_eq!(index.len(), 2);
    assert_eq!(index.get(0).unwrap(), 1);
    assert_eq!(index.get(1).unwrap(), 3);
}
