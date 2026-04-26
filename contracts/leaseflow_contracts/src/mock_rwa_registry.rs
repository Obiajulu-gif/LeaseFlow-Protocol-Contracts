use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, 
    Address, Env, String, BytesN, Vec
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockAsset {
    pub asset_id: u128,
    pub owner: Address,
    pub is_frozen: bool,
    pub freezer: Option<Address>,
    pub freeze_proof: Option<BytesN<32>>,
    pub metadata_uri: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryInfo {
    pub registry_id: BytesN<32>,
    pub version: u32,
    pub is_whitelisted: bool,
    pub supported_standards: Vec<String>,
}

#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockRegistryError {
    AssetNotFound = 1,
    NotOwner = 2,
    AlreadyFrozen = 3,
    NotFrozen = 4,
    InvalidFreezer = 5,
    InvalidFreezeProof = 6,
}

#[contractevent]
pub struct AssetFrozen {
    pub asset_id: u128,
    pub freezer: Address,
    pub freeze_proof: BytesN<32>,
    pub timestamp: u64,
}

#[contractevent]
pub struct AssetThawed {
    pub asset_id: u128,
    pub freezer: Address,
    pub timestamp: u64,
}

#[contract]
pub struct MockRWARegistry;

#[contractimpl]
impl MockRWARegistry {
    pub fn new(env: Env, admin: Address) {
        // Initialize registry with admin
        let registry_info = RegistryInfo {
            registry_id: BytesN::from_array(&env, &[1; 32]),
            version: 1,
            is_whitelisted: true,
            supported_standards: Vec::from_array(&env, [
                String::from_str(&env, "ERC721"),
                String::from_str(&env, "ERC1155"),
                String::from_str(&env, "RWA-TOKEN")
            ]),
        };
        
        env.storage().instance().set(&0u64, &registry_info);
        env.storage().instance().set(&1u64, &admin);
    }

    pub fn get_registry_info(env: Env) -> Result<RegistryInfo, u32> {
        let registry_info: RegistryInfo = env.storage().instance()
            .get(&0u64)
            .ok_or(MockRegistryError::AssetNotFound as u32)?;
        Ok(registry_info)
    }

    pub fn verify_ownership(env: Env, asset_id: u128, claimed_owner: Address) -> bool {
        if let Some(asset) = Self::get_asset(&env, asset_id) {
            asset.owner == claimed_owner
        } else {
            false
        }
    }

    pub fn freeze_asset(env: Env, asset_id: u128, freezer: Address) -> Result<BytesN<32>, u32> {
        let mut asset = Self::get_asset(&env, asset_id)
            .ok_or(MockRegistryError::AssetNotFound as u32)?;

        if asset.is_frozen {
            return Err(MockRegistryError::AlreadyFrozen as u32);
        }

        // Generate freeze proof (mock implementation)
        let freeze_proof = Self::generate_freeze_proof(&env, asset_id, freezer.clone());
        
        asset.is_frozen = true;
        asset.freezer = Some(freezer.clone());
        asset.freeze_proof = Some(freeze_proof.clone());

        // Update asset
        env.storage().instance().set(&asset_id, &asset);

        // Emit event
        AssetFrozen {
            asset_id,
            freezer,
            freeze_proof: freeze_proof.clone(),
            timestamp: env.ledger().timestamp(),
        }.publish(&env);

        Ok(freeze_proof)
    }

    pub fn thaw_asset(env: Env, asset_id: u128, freezer: Address, freeze_proof: BytesN<32>) -> Result<(), u32> {
        let mut asset = Self::get_asset(&env, asset_id)
            .ok_or(MockRegistryError::AssetNotFound as u32)?;

        if !asset.is_frozen {
            return Err(MockRegistryError::NotFrozen as u32);
        }

        if asset.freezer != Some(freezer.clone()) {
            return Err(MockRegistryError::InvalidFreezer as u32);
        }

        if asset.freeze_proof != Some(freeze_proof) {
            return Err(MockRegistryError::InvalidFreezeProof as u32);
        }

        // Thaw the asset
        asset.is_frozen = false;
        asset.freezer = None;
        asset.freeze_proof = None;

        // Update asset
        env.storage().instance().set(&asset_id, &asset);

        // Emit event
        AssetThawed {
            asset_id,
            freezer,
            timestamp: env.ledger().timestamp(),
        }.publish(&env);

        Ok(())
    }

    pub fn is_asset_frozen(env: Env, asset_id: u128) -> bool {
        if let Some(asset) = Self::get_asset(&env, asset_id) {
            asset.is_frozen
        } else {
            false
        }
    }

    // Helper functions for testing
    pub fn mint_asset(env: Env, asset_id: u128, owner: Address, metadata_uri: String) -> Result<(), u32> {
        let admin: Address = env.storage().instance()
            .get(&1u64)
            .ok_or(MockRegistryError::AssetNotFound as u32)?;
        
        admin.require_auth();

        let asset = MockAsset {
            asset_id,
            owner,
            is_frozen: false,
            freezer: None,
            freeze_proof: None,
            metadata_uri,
        };

        env.storage().instance().set(&asset_id, &asset);
        Ok(())
    }

    pub fn transfer_asset(env: Env, asset_id: u128, from: Address, to: Address) -> Result<(), u32> {
        let mut asset = Self::get_asset(&env, asset_id)
            .ok_or(MockRegistryError::AssetNotFound as u32)?;

        if asset.owner != from {
            return Err(MockRegistryError::NotOwner as u32);
        }

        if asset.is_frozen {
            return Err(MockRegistryError::AlreadyFrozen as u32);
        }

        from.require_auth();

        asset.owner = to;
        env.storage().instance().set(&asset_id, &asset);
        Ok(())
    }

    // Malicious registry functions for testing security
    pub fn set_whitelisted(env: Env, whitelisted: bool) {
        let admin: Address = env.storage().instance()
            .get(&1u64)
            .expect("Admin not set");
        
        admin.require_auth();

        let mut registry_info: RegistryInfo = env.storage().instance()
            .get(&0u64)
            .expect("Registry info not set");
        
        registry_info.is_whitelisted = whitelisted;
        env.storage().instance().set(&0u64, &registry_info);
    }

    pub fn spoof_ownership(env: Env, asset_id: u128, claimed_owner: Address) -> bool {
        // Malicious function that always returns true for ownership verification
        // Used to test security vulnerabilities
        true
    }

    // Private helper functions
    fn get_asset(env: &Env, asset_id: u128) -> Option<MockAsset> {
        env.storage().instance().get(&asset_id)
    }

    fn generate_freeze_proof(env: &Env, asset_id: u128, freezer: Address) -> BytesN<32> {
        // Mock implementation: generate deterministic freeze proof
        let mut data = [0u8; 32];
        data[0..16].copy_from_slice(&asset_id.to_be_bytes());
        data[16..32].copy_from_slice(&freezer.to_string().to_array()[..16]);
        BytesN::from_array(env, &data)
    }
}
