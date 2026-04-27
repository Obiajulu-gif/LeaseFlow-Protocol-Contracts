use soroban_sdk::{contracttype, symbol_short, BytesN, Env, Symbol};

/// Standardized Event Actions
pub const ACTION_CREATED: Symbol = symbol_short!("CREATED");
pub const ACTION_RENT_PAID: Symbol = symbol_short!("RENT_PAID");
pub const ACTION_SLASHED: Symbol = symbol_short!("SLASHED");
pub const ACTION_EVICTED: Symbol = symbol_short!("EVICTED");
pub const ACTION_TERMINATED: Symbol = symbol_short!("TERMINATED");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardEventPayload {
    pub timestamp: u64,
    pub data_hash: BytesN<32>, // Cryptographic hash pointing to off-chain data (ensures NO PII)
    pub amount: i128,          // Relevant token amount associated with the event
}

pub trait EventEmission {
    /// Emits a standardized event mapping exactly to: [Action, LeaseID, ReasonCode] -> [Timestamp, DataHash, Amount]
    fn emit_standard_event(
        env: &Env,
        action: Symbol,
        lease_id: u64,
        reason_code: u32,
        data_hash: BytesN<32>,
        amount: i128,
    );
}

pub struct LeaseFlowEvents;

impl EventEmission for LeaseFlowEvents {
    fn emit_standard_event(
        env: &Env,
        action: Symbol,
        lease_id: u64,
        reason_code: u32,
        data_hash: BytesN<32>,
        amount: i128,
    ) {
        let topics = (action, lease_id, reason_code);
        
        let payload = StandardEventPayload {
            timestamp: env.ledger().timestamp(),
            data_hash,
            amount,
        };

        // Publish the standardized event to the Soroban environment
        env.events().publish(topics, payload);
    }
}