#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Symbol, BytesN, String,
};

macro_rules! require {
    ($condition:expr, $error_msg:expr) => {
        if !$condition {
            panic!($error_msg);
        }
    };
}

// ── Rate helpers ──────────────────────────────────────────────────────────────

/// Describes the time unit in which a monetary rate is expressed by the caller.
/// All rates are normalised to **per-second** before being stored on-chain, so
/// internal arithmetic never needs to know the original unit.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    PerSecond,
    PerHour,
    PerDay,
}


pub fn to_per_second(rate: i128, rate_type: RateType) -> i128 {
    match rate_type {
        RateType::PerSecond => rate,
        RateType::PerHour   => rate / 3_600,
        RateType::PerDay    => rate / 86_400,
    }
}

/// Seconds of lease time granted per unit of funds added (1 day per unit).
pub const SECS_PER_UNIT: u64 = 86_400;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeaseStatus {
    Pending,
    Active,
    Expired,
    Disputed,
}

mod nft_contract {
    use soroban_sdk::{contractclient, Address, Env};

    #[allow(dead_code)]
    #[contractclient(name = "NftClient")]
    pub trait NftInterface {
        fn transfer_from(env: Env, spender: Address, from: Address, to: Address, token_id: u128);
    }
}

/// Core lease record stored on-chain.
///
/// All rate fields (`rent_per_sec`, `late_fee_per_sec`) are normalised to
/// **per-second** by [`to_per_second`] at creation time — callers pass the
/// human-friendly value together with a [`RateType`] and conversion happens
/// once in the contract entry points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub landlord: Address,
    pub tenant: Address,
    /// Per-second rent rate (normalised from the caller-supplied `RateType`).
    pub rent_per_sec: i128,
    pub deposit_amount: i128,
    pub start_date: u64,
    pub end_date: u64,
    pub property_uri: String,
    pub status: LeaseStatus,
    pub nft_contract: Option<Address>,
    pub token_id: Option<u128>,
    pub active: bool,
    pub grace_period_end: u64,
    /// One-time flat late fee applied the first second rent is overdue.
    pub late_fee_flat: i128,
    /// Per-second late fee (normalised from the caller-supplied `RateType`).
    pub late_fee_per_sec: i128,
    pub debt: i128,
    pub flat_fee_applied: bool,
    /// Total seconds of lateness for which the per-second fee has been charged.
    pub seconds_late_charged: u64,
    pub rent_paid: i128,
    pub expiry_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseAmendment {
    /// Provide a new per-second rent rate, already normalised via
    /// [`to_per_second`] off-chain (or pass `None` to keep current value).
    pub new_rent_per_sec: Option<i128>,
    pub new_end_date: Option<u64>,
    pub landlord_signature: BytesN<32>,
    pub tenant_signature: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositReleasePartial {
    pub tenant_amount: i128,
    pub landlord_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DepositRelease {
    FullRefund,
    PartialRefund(DepositReleasePartial),
    Disputed,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct LeaseContract;

#[contractimpl]
impl LeaseContract {
    
    pub fn create_lease(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        rent_rate_type: RateType,
        duration: u64,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_amount: i128,
        late_fee_rate_type: RateType,
    ) -> Symbol {
        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);

        let lease = Lease {
            landlord,
            tenant,
            //  normalise to per-second 
            rent_per_sec:      to_per_second(rent_amount,     rent_rate_type),
            late_fee_per_sec:  to_per_second(late_fee_amount, late_fee_rate_type),
            //
            deposit_amount: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Pending,
            nft_contract: None,
            token_id: None,
            active: true,
            grace_period_end,
            late_fee_flat,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time,
        };

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("pending")
    }

    /// Creates a lease **and** immediately transfers an NFT from landlord to
    /// tenant.  Rate inputs follow the same `RateType` convention as
    /// [`create_lease`].
    pub fn create_lease_with_nft(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        tenant: Address,
        rent_amount: i128,
        rent_rate_type: RateType,
        duration: u64,
        grace_period_end: u64,
        late_fee_flat: i128,
        late_fee_amount: i128,
        late_fee_rate_type: RateType,
        nft_contract_addr: Address,
        token_id: u128,
    ) -> Symbol {
        landlord.require_auth();

        let nft_client = nft_contract::NftClient::new(&env, &nft_contract_addr);
        nft_client.transfer_from(
            &env.current_contract_address(),
            &landlord,
            &tenant,
            &token_id,
        );

        let now = env.ledger().timestamp();
        let expiry_time = now.saturating_add(duration);

        let lease = Lease {
            landlord,
            tenant,
            rent_per_sec:      to_per_second(rent_amount,     rent_rate_type),
            late_fee_per_sec:  to_per_second(late_fee_amount, late_fee_rate_type),
            deposit_amount: 0,
            start_date: now,
            end_date: expiry_time,
            property_uri: String::from_str(&env, ""),
            status: LeaseStatus::Active,
            nft_contract: Some(nft_contract_addr),
            token_id: Some(token_id),
            active: true,
            grace_period_end,
            late_fee_flat,
            debt: 0,
            flat_fee_applied: false,
            seconds_late_charged: 0,
            rent_paid: 0,
            expiry_time,
        };

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("created")
    }

    /// Activates a pending lease after the security deposit has been received.
    pub fn activate_lease(env: Env, lease_id: Symbol, tenant: Address) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.tenant == tenant, "Unauthorized: Only tenant can activate lease");
        require!(lease.status == LeaseStatus::Pending, "Lease is not in pending state");

        lease.status = LeaseStatus::Active;

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("active")
    }

    /// Updates the property metadata URI.
    pub fn update_property_uri(
        env: Env,
        lease_id: Symbol,
        landlord: Address,
        property_uri: String,
    ) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.landlord == landlord,
            "Unauthorized: Only landlord can update property URI"
        );
        lease.property_uri = property_uri;

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("updated")
    }

    /// Amends a lease with both landlord and tenant signatures.
    /// `amendment.new_rent_per_sec` should be pre-normalised by the caller
    /// using [`to_per_second`] if needed.
    pub fn amend_lease(env: Env, lease_id: Symbol, amendment: LeaseAmendment) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(lease.status == LeaseStatus::Active, "Can only amend active leases");

        // Signatures are trusted here; a production implementation would
        // verify `amendment.landlord_signature` and `amendment.tenant_signature`.
        if let Some(new_rent) = amendment.new_rent_per_sec {
            lease.rent_per_sec = new_rent;
        }
        if let Some(new_end_date) = amendment.new_end_date {
            lease.end_date = new_end_date;
        }

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("amended")
    }

    /// Releases the security deposit according to `release_type`.
    pub fn release_deposit(
        env: Env,
        lease_id: Symbol,
        release_type: DepositRelease,
    ) -> Symbol {
        let lease = Self::get_lease(env.clone(), lease_id.clone());

        require!(
            lease.status == LeaseStatus::Active || lease.status == LeaseStatus::Expired,
            "Can only release deposit from active or expired leases"
        );

        match release_type {
            DepositRelease::FullRefund => symbol_short!("full_ref"),
            DepositRelease::PartialRefund(partial) => {
                require!(
                    partial.tenant_amount + partial.landlord_amount == lease.deposit_amount,
                    "Amounts must sum to total deposit"
                );
                symbol_short!("partial")
            }
            DepositRelease::Disputed => {
                let mut updated = lease;
                updated.status = LeaseStatus::Disputed;
                env.storage().instance().set(&lease_id, &updated);
                symbol_short!("disputed")
            }
        }
    }

    /// Returns the lease stored under `lease_id`.
    pub fn get_lease(env: Env, lease_id: Symbol) -> Lease {
        env.storage()
            .instance()
            .get(&lease_id)
            .expect("Lease not found")
    }

    /// Processes a rent payment.
    ///
    /// Late fees are accrued in **per-second** terms using the stored
    /// `late_fee_per_sec` — no hardcoded 86 400 divisor is needed.
    /// The monthly rent threshold is derived from `rent_per_sec × 2_592_000`
    /// (30 × 86 400 seconds).
    pub fn pay_rent(env: Env, lease_id: Symbol, payment_amount: i128) -> Symbol {
        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        require!(lease.active, "Lease is not active");

        let current_time = env.ledger().timestamp();

        // ── Accrue late fees (all in per-second units) ────────────────────
        if current_time > lease.grace_period_end {
            let seconds_late = current_time - lease.grace_period_end;

            // One-time flat fee applied on the first overdue second.
            if !lease.flat_fee_applied {
                lease.debt += lease.late_fee_flat;
                lease.flat_fee_applied = true;
            }

            // Per-second fee: only charge newly elapsed seconds.
            if seconds_late > lease.seconds_late_charged {
                let newly_accrued = seconds_late - lease.seconds_late_charged;
                lease.debt += (newly_accrued as i128) * lease.late_fee_per_sec;
                lease.seconds_late_charged = seconds_late;
            }
        }

        // ── Apply payment: clear debt first, then current-month rent ──────
        let mut remaining = payment_amount;

        if lease.debt > 0 {
            if remaining >= lease.debt {
                remaining -= lease.debt;
                lease.debt = 0;
            } else {
                lease.debt -= remaining;
                remaining = 0;
            }
        }

        if remaining > 0 {
            lease.rent_paid += remaining;

            // Monthly rent = per-second rate × seconds-in-30-days.
            let monthly_rent = lease.rent_per_sec.saturating_mul(2_592_000);
            if lease.rent_paid >= monthly_rent {
                lease.rent_paid -= monthly_rent;
                lease.grace_period_end = lease.grace_period_end.saturating_add(2_592_000);
                lease.flat_fee_applied = false;
                lease.seconds_late_charged = 0;
            }
        }

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("paid")
    }

    /// Adds funds to an existing lease, extending `expiry_time` proportionally.
    /// Each unit of `amount` extends the lease by [`SECS_PER_UNIT`] seconds.
    /// Requires authorisation from the tenant.
    pub fn add_funds(env: Env, lease_id: Symbol, amount: i128) -> Symbol {
        assert!(amount > 0, "amount must be positive");

        let mut lease = Self::get_lease(env.clone(), lease_id.clone());
        lease.tenant.require_auth();

        let extra_secs = (amount as u64).saturating_mul(SECS_PER_UNIT);
        lease.expiry_time = lease.expiry_time.saturating_add(extra_secs);

        env.storage().instance().set(&lease_id, &lease);
        symbol_short!("extended")
    }
}

mod test;