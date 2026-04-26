//! Ledger Rent Sweeper for Expired Lease Proposals (Issue #132)
//!
//! Implements `sweep_expired_proposals`, a permissionless function callable by any
//! network relayer to clean up pending lease proposals that never received a security
//! deposit within the initialization timeout window.
//!
//! # Security
//! - Only `Pending` leases whose `start_date` + `INIT_TIMEOUT_SECS` < `now` are eligible.
//! - Active, Disputed, Terminated, or DaoArbitration leases are **never** touched.
//! - Partial co-signer funds are refunded before the record is deleted.
//! - A small gas bounty is paid to the relayer from the platform fee vault.

use soroban_sdk::{contractevent, Address, Env, Symbol};

use crate::{
    load_lease_instance_by_id, token_contract, DataKey, LeaseError, LeaseStatus,
};

/// A pending lease that never received a deposit is considered expired after 7 days.
pub const INIT_TIMEOUT_SECS: u64 = 7 * 24 * 60 * 60;

/// Relayer bounty: 0.5% of the platform fee amount.
const BOUNTY_BPS: i128 = 50;

/// Emitted when a stale pending proposal is swept from storage.
#[contractevent]
pub struct ProposalSwept {
    /// The lease ID that was removed.
    pub lease_id: u64,
    /// The relayer that triggered the sweep and received the bounty.
    pub swept_by: Address,
    /// Unix timestamp at which the sweep occurred.
    pub swept_at: u64,
}

/// Sweep a single expired pending lease proposal from persistent storage.
///
/// # Parameters
/// - `env`      – Soroban execution environment.
/// - `relayer`  – Address of the caller; receives the gas bounty.
/// - `lease_id` – ID of the candidate lease to sweep.
///
/// # Returns
/// `Ok(())` on success.
///
/// # Errors
/// - [`LeaseError::LeaseNotFound`]   – No lease exists for `lease_id`.
/// - [`LeaseError::InvalidState`]    – Lease is not in `Pending` status.
/// - [`LeaseError::LeaseNotExpired`] – Initialization timeout has not elapsed yet.
///
/// # Authorization
/// None required — permissionless by design so any relayer can call it.
pub fn sweep_expired_proposals(
    env: &Env,
    relayer: Address,
    lease_id: u64,
) -> Result<(), LeaseError> {
    let lease = load_lease_instance_by_id(env, lease_id).ok_or(LeaseError::LeaseNotFound)?;

    // Guard: only Pending leases are eligible — never touch active/disputed/terminated.
    if lease.status != LeaseStatus::Pending {
        return Err(LeaseError::InvalidState);
    }

    let now = env.ledger().timestamp();
    let deadline = lease.start_date.saturating_add(INIT_TIMEOUT_SECS);

    if now < deadline {
        return Err(LeaseError::LeaseNotExpired);
    }

    // Refund any partial security deposit back to the tenant.
    if lease.security_deposit > 0 {
        let token = token_contract::TokenClient::new(env, &lease.payment_token);
        token.transfer(
            &env.current_contract_address(),
            &lease.tenant,
            &lease.security_deposit,
        );
    }

    // Pay the relayer a small bounty from the platform fee vault if configured.
    if let (Some(fee_amount), Some(fee_token), Some(fee_recipient)) = (
        env.storage()
            .instance()
            .get::<DataKey, i128>(&DataKey::PlatformFeeAmount),
        env.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PlatformFeeToken),
        env.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PlatformFeeRecipient),
    ) {
        let bounty = fee_amount.saturating_mul(BOUNTY_BPS) / 10_000;
        if bounty > 0 {
            let token = token_contract::TokenClient::new(env, &fee_token);
            token.transfer(&fee_recipient, &relayer, &bounty);
        }
    }

    // Delete the stale record from persistent storage.
    env.storage()
        .persistent()
        .remove(&DataKey::LeaseInstance(lease_id));

    // Remove from the active leases index.
    crate::LeaseContract::remove_from_active_leases_index(env, lease_id)?;

    ProposalSwept {
        lease_id,
        swept_by: relayer,
        swept_at: now,
    }
    .publish(env);

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        save_lease_instance, CreateLeaseParams, DataKey, DepositStatus, LeaseContract,
        LeaseContractClient, LeaseInstance, LeaseStatus, MaintenanceStatus,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        vec, Address, Env, String,
    };

    const START: u64 = 1_711_929_600;

    fn make_pending_lease(env: &Env, landlord: Address, tenant: Address, start: u64) -> LeaseInstance {
        let token = Address::generate(env);
        LeaseInstance {
            landlord,
            tenant,
            rent_amount: 1_000,
            deposit_amount: 500,
            security_deposit: 0,
            start_date: start,
            end_date: start + 30 * 86_400,
            property_uri: String::from_str(env, "ipfs://test"),
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            active: true,
            rent_paid: 0,
            expiry_time: start + 30 * 86_400,
            buyout_price: None,
            cumulative_payments: 0,
            debt: 0,
            rent_paid_through: 0,
            deposit_status: DepositStatus::Held,
            rent_per_sec: 0,
            grace_period_end: start + 30 * 86_400,
            late_fee_flat: 0,
            late_fee_per_sec: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            withdrawal_address: None,
            rent_withdrawn: 0,
            arbitrators: soroban_sdk::Vec::new(env),
            maintenance_status: MaintenanceStatus::None,
            withheld_rent: 0,
            repair_proof_hash: None,
            inspector: None,
            paused: false,
            pause_reason: None,
            paused_at: None,
            pause_initiator: None,
            total_paused_duration: 0,
            rent_pull_authorized_amount: None,
            last_rent_pull_timestamp: None,
            billing_cycle_duration: 2_592_000,
            yield_delegation_enabled: false,
            yield_accumulated: 0,
            equity_balance: 0,
            equity_percentage_bps: 0,
            had_late_payment: false,
            has_pet: false,
            pet_deposit_amount: 0,
            pet_rent_amount: 0,
            payment_token: token,
        }
    }

    #[test]
    fn test_sweep_expired_proposal_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = START);

        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        let relayer = Address::generate(&env);

        let lease = make_pending_lease(&env, landlord, tenant, START);
        save_lease_instance(&env, 1, &lease);

        // Advance time past the 7-day timeout.
        env.ledger()
            .with_mut(|l| l.timestamp = START + INIT_TIMEOUT_SECS + 1);

        let result = sweep_expired_proposals(&env, relayer, 1);
        assert!(result.is_ok());

        // Lease must be gone from storage.
        assert!(load_lease_instance_by_id(&env, 1).is_none());
    }

    #[test]
    fn test_sweep_before_timeout_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = START);

        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        let relayer = Address::generate(&env);

        let lease = make_pending_lease(&env, landlord, tenant, START);
        save_lease_instance(&env, 2, &lease);

        // Only 3 days have passed — not yet expired.
        env.ledger()
            .with_mut(|l| l.timestamp = START + 3 * 86_400);

        let result = sweep_expired_proposals(&env, relayer, 2);
        assert_eq!(result, Err(LeaseError::LeaseNotExpired));
    }

    #[test]
    fn test_sweep_active_lease_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = START);

        let landlord = Address::generate(&env);
        let tenant = Address::generate(&env);
        let relayer = Address::generate(&env);

        let mut lease = make_pending_lease(&env, landlord, tenant, START);
        lease.status = LeaseStatus::Active;
        save_lease_instance(&env, 3, &lease);

        env.ledger()
            .with_mut(|l| l.timestamp = START + INIT_TIMEOUT_SECS + 1);

        let result = sweep_expired_proposals(&env, relayer, 3);
        assert_eq!(result, Err(LeaseError::InvalidState));
    }

    #[test]
    fn test_sweep_nonexistent_lease_fails() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = START);

        let relayer = Address::generate(&env);
        let result = sweep_expired_proposals(&env, relayer, 999);
        assert_eq!(result, Err(LeaseError::LeaseNotFound));
    }
}
