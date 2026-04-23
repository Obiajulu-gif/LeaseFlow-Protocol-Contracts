//! DEX Integration and Secondary Market Support
//! 
//! This module provides seamless integration with decentralized exchanges
//! for trading lessor rights NFTs on secondary markets.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError, LeaseStatus,
    lessor_rights_nft::{LessorRightsNFT, LessorRightsNFTMetadata, NFTDataKey},
    lease_payment_router::LeaseContract as PaymentRouter
};

/// DEX integration configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DEXIntegrationConfig {
    pub dex_contract: Address,
    pub token_contract: Address,
    pub fee_bps: u32,
    pub min_price: i128,
    pub max_price: i128,
    pub enabled: bool,
}

/// NFT listing on DEX
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTListing {
    pub token_id: u128,
    pub seller: Address,
    pub price: i128,
    pub listed_at: u64,
    pub expires_at: Option<u64>,
    pub lease_id: u64,
    pub yield_projection: i128, // Projected monthly yield
    pub active: bool,
}

/// Market data for NFT
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTMarketData {
    pub token_id: u128,
    pub lease_id: u64,
    pub current_price: i128,
    pub last_sale_price: i128,
    pub last_sale_timestamp: u64,
    pub price_history: Vec<PricePoint>,
    pub yield_history: Vec<YieldPoint>,
    pub total_volume: i128,
    pub sale_count: u32,
}

/// Price point for history
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricePoint {
    pub price: i128,
    pub timestamp: u64,
    pub volume: i128,
}

/// Yield point for history
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldPoint {
    pub yield_amount: i128,
    pub timestamp: u64,
    pub holder: Address,
}

/// DEX integration events
#[contractevent]
pub struct NFTListed {
    pub token_id: u128,
    pub seller: Address,
    pub price: i128,
    pub lease_id: u64,
    pub listed_at: u64,
}

#[contractevent]
pub struct NFTSold {
    pub token_id: u128,
    pub seller: Address,
    pub buyer: Address,
    pub price: i128,
    pub fees: i128,
    pub lease_id: u64,
    pub sold_at: u64,
}

#[contractevent]
pub struct NFTDelisted {
    pub token_id: u128,
    pub seller: Address,
    pub delisted_at: u64,
}

#[contractevent]
pub struct MarketDataUpdated {
    pub token_id: u128,
    pub lease_id: u64,
    pub new_price: i128,
    pub update_timestamp: u64,
}

/// DEX integration errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DEXError {
    NFTNotFound = 4001,
    NotAuthorized = 4002,
    InvalidPrice = 4003,
    ListingExpired = 4004,
    InsufficientBalance = 4005,
    DEXNotEnabled = 4006,
    TransferFailed = 4007,
    MarketDataCorrupted = 4008,
    YieldProjectionFailed = 4009,
    CrossContractCallFailed = 4010,
}

/// DEX Integration Manager
pub struct DEXIntegrationManager;

impl DEXIntegrationManager {
    /// List NFT on secondary market
    pub fn list_nft_on_dex(
        env: Env,
        token_id: u128,
        seller: Address,
        price: i128,
        duration_days: Option<u32>,
    ) -> Result<(), DEXError> {
        // Verify NFT exists and seller is owner
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id)
            .ok_or(DEXError::NFTNotFound)?;
        
        if metadata.current_holder != seller {
            return Err(DEXError::NotAuthorized);
        }
        
        // Verify DEX is enabled
        let dex_config = Self::get_dex_config(&env)?;
        if !dex_config.enabled {
            return Err(DEXError::DEXNotEnabled);
        }
        
        // Validate price
        if price < dex_config.min_price || price > dex_config.max_price {
            return Err(DEXError::InvalidPrice);
        }
        
        // Check if NFT is locked
        if Self::is_nft_locked(&env, metadata.lease_id) {
            return Err(DEXError::NotAuthorized);
        }
        
        // Calculate yield projection
        let yield_projection = Self::calculate_yield_projection(&env, &metadata)?;
        
        // Create listing
        let expires_at = duration_days.map(|days| env.ledger().timestamp() + (days as u64 * 24 * 60 * 60));
        
        let listing = NFTListing {
            token_id,
            seller: seller.clone(),
            price,
            listed_at: env.ledger().timestamp(),
            expires_at,
            lease_id: metadata.lease_id,
            yield_projection,
            active: true,
        };
        
        // Store listing
        Self::store_listing(&env, token_id, &listing)?;
        
        // Update market data
        Self::update_market_data_on_listing(&env, token_id, price)?;
        
        // Emit listing event
        NFTListed {
            token_id,
            seller,
            price,
            lease_id: metadata.lease_id,
            listed_at: listing.listed_at,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Buy NFT from secondary market
    pub fn buy_nft_from_dex(
        env: Env,
        token_id: u128,
        buyer: Address,
        max_price: i128,
    ) -> Result<(), DEXError> {
        // Get listing
        let listing = Self::get_listing(&env, token_id)?;
        
        if !listing.active {
            return Err(DEXError::ListingExpired);
        }
        
        // Check if listing expired
        if let Some(expires_at) = listing.expires_at {
            if env.ledger().timestamp() > expires_at {
                return Err(DEXError::ListingExpired);
            }
        }
        
        // Check price
        if listing.price > max_price {
            return Err(DEXError::InvalidPrice);
        }
        
        // Verify DEX is enabled
        let dex_config = Self::get_dex_config(&env)?;
        if !dex_config.enabled {
            return Err(DEXError::DEXNotEnabled);
        }
        
        // Calculate fees
        let fees = (listing.price * dex_config.fee_bps as i128) / 10000;
        let seller_proceeds = listing.price - fees;
        
        // In a real implementation, this would handle token transfers
        // For now, we'll simulate the transfer
        
        // Transfer NFT
        LessorRightsNFT::transfer_nft(env.clone(), token_id, listing.seller.clone(), buyer.clone())?;
        
        // Update payment routing
        PaymentRouter::update_payment_routing_on_nft_transfer(
            env.clone(),
            listing.lease_id,
            listing.seller.clone(),
            buyer.clone(),
        ).map_err(|_| DEXError::TransferFailed)?;
        
        // Update market data
        Self::update_market_data_on_sale(&env, token_id, listing.price, fees)?;
        
        // Remove listing
        Self::remove_listing(&env, token_id)?;
        
        // Emit sale event
        NFTSold {
            token_id,
            seller: listing.seller,
            buyer: buyer.clone(),
            price: listing.price,
            fees,
            lease_id: listing.lease_id,
            sold_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Delist NFT from market
    pub fn delist_nft(
        env: Env,
        token_id: u128,
        seller: Address,
    ) -> Result<(), DEXError> {
        let listing = Self::get_listing(&env, token_id)?;
        
        if listing.seller != seller {
            return Err(DEXError::NotAuthorized);
        }
        
        // Remove listing
        Self::remove_listing(&env, token_id)?;
        
        // Emit delist event
        NFTDelisted {
            token_id,
            seller,
            delisted_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Update listing price
    pub fn update_listing_price(
        env: Env,
        token_id: u128,
        seller: Address,
        new_price: i128,
    ) -> Result<(), DEXError> {
        let mut listing = Self::get_listing(&env, token_id)?;
        
        if listing.seller != seller {
            return Err(DEXError::NotAuthorized);
        }
        
        // Validate new price
        let dex_config = Self::get_dex_config(&env)?;
        if new_price < dex_config.min_price || new_price > dex_config.max_price {
            return Err(DEXError::InvalidPrice);
        }
        
        // Update listing
        listing.price = new_price;
        Self::store_listing(&env, token_id, &listing)?;
        
        // Update market data
        Self::update_market_data_on_price_update(&env, token_id, new_price)?;
        
        // Emit price update event
        MarketDataUpdated {
            token_id,
            lease_id: listing.lease_id,
            new_price,
            update_timestamp: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get market data for NFT
    pub fn get_market_data(env: Env, token_id: u128) -> Result<NFTMarketData, DEXError> {
        env.storage()
            .persistent()
            .get::<_, NFTMarketData>(&NFTDataKey::LeaseInstance(token_id)) // Reuse existing key
            .ok_or(DEXError::MarketDataCorrupted)
    }
    
    /// Get all active listings
    pub fn get_active_listings(env: Env) -> Vec<NFTListing> {
        let mut listings = Vec::new(&env);
        
        // In a real implementation, this would scan for active listings
        // For now, return empty vector
        listings
    }
    
    /// Get market statistics
    pub fn get_market_statistics(env: Env) -> MarketStatistics {
        MarketStatistics {
            total_listings: 0,
            active_listings: 0,
            total_volume_24h: 0,
            average_price: 0,
            highest_price: 0,
            lowest_price: i128::MAX,
            total_yield_distributed: 0,
            last_updated: env.ledger().timestamp(),
        }
    }
    
    /// Configure DEX integration
    pub fn configure_dex(
        env: Env,
        admin: Address,
        dex_contract: Address,
        token_contract: Address,
        fee_bps: u32,
        min_price: i128,
        max_price: i128,
    ) -> Result<(), DEXError> {
        // Verify admin authorization
        if !Self::is_admin(&env, &admin) {
            return Err(DEXError::NotAuthorized);
        }
        
        let config = DEXIntegrationConfig {
            dex_contract,
            token_contract,
            fee_bps,
            min_price,
            max_price,
            enabled: true,
        };
        
        // Store configuration
        env.storage()
            .persistent()
            .set(&NFTDataKey::Admin, &config);
        
        Ok(())
    }
    
    /// Enable/disable DEX integration
    pub fn toggle_dex_integration(env: Env, admin: Address, enabled: bool) -> Result<(), DEXError> {
        if !Self::is_admin(&env, &admin) {
            return Err(DEXError::NotAuthorized);
        }
        
        let mut config = Self::get_dex_config(&env)?;
        config.enabled = enabled;
        
        env.storage()
            .persistent()
            .set(&NFTDataKey::Admin, &config);
        
        Ok(())
    }
    
    // Helper methods
    
    fn get_dex_config(env: &Env) -> Result<DEXIntegrationConfig, DEXError> {
        env.storage()
            .persistent()
            .get::<_, DEXIntegrationConfig>(&NFTDataKey::Admin)
            .ok_or(DEXError::DEXNotEnabled)
    }
    
    fn is_admin(env: &Env, address: &Address) -> bool {
        // Check if address is contract admin
        if let Some(admin) = env.storage().instance().get::<_, Address>(&NFTDataKey::Admin) {
            admin == *address
        } else {
            false
        }
    }
    
    fn is_nft_locked(env: &Env, lease_id: u64) -> bool {
        env.storage()
            .persistent()
            .has(&NFTDataKey::NFTIndestructibilityLock(lease_id))
    }
    
    fn calculate_yield_projection(env: &Env, metadata: &LessorRightsNFTMetadata) -> Result<i128, DEXError> {
        // Calculate monthly yield projection based on lease terms
        let remaining_months = if metadata.lease_end > env.ledger().timestamp() {
            (metadata.lease_end - env.ledger().timestamp()) / (30 * 24 * 60 * 60)
        } else {
            0
        };
        
        if remaining_months == 0 {
            return Ok(0);
        }
        
        // Project monthly yield (rent amount)
        Ok(metadata.monthly_rent)
    }
    
    fn store_listing(env: &Env, token_id: u128, listing: &NFTListing) -> Result<(), DEXError> {
        env.storage()
            .persistent()
            .set(&NFTDataKey::LeaseInstance(token_id), listing);
        
        // Set TTL
        let ttl = if let Some(expires_at) = listing.expires_at {
            expires_at - env.ledger().timestamp()
        } else {
            365 * 24 * 60 * 60 // 1 year
        };
        
        env.storage()
            .persistent()
            .extend_ttl(&NFTDataKey::LeaseInstance(token_id), ttl, ttl);
        
        Ok(())
    }
    
    fn get_listing(env: &Env, token_id: u128) -> Result<NFTListing, DEXError> {
        env.storage()
            .persistent()
            .get::<_, NFTListing>(&NFTDataKey::LeaseInstance(token_id))
            .ok_or(DEXError::NFTNotFound)
    }
    
    fn remove_listing(env: &Env, token_id: u128) -> Result<(), DEXError> {
        env.storage()
            .persistent()
            .remove(&NFTDataKey::LeaseInstance(token_id));
        
        Ok(())
    }
    
    fn update_market_data_on_listing(env: &Env, token_id: u128, price: i128) -> Result<(), DEXError> {
        let mut market_data = env.storage()
            .persistent()
            .get::<_, NFTMarketData>(&NFTDataKey::LeaseInstance(token_id))
            .unwrap_or_else(|| {
                // Create new market data
                NFTMarketData {
                    token_id,
                    lease_id: 0, // Will be updated below
                    current_price: price,
                    last_sale_price: 0,
                    last_sale_timestamp: 0,
                    price_history: Vec::new(env),
                    yield_history: Vec::new(env),
                    total_volume: 0,
                    sale_count: 0,
                }
            });
        
        // Update current price
        market_data.current_price = price;
        
        // Add price point to history
        let price_point = PricePoint {
            price,
            timestamp: env.ledger().timestamp(),
            volume: 0, // No volume on listing
        };
        market_data.price_history.push_back(price_point);
        
        // Store updated market data
        env.storage()
            .persistent()
            .set(&NFTDataKey::LeaseInstance(token_id), &market_data);
        
        Ok(())
    }
    
    fn update_market_data_on_sale(env: &Env, token_id: u128, price: i128, fees: i128) -> Result<(), DEXError> {
        let mut market_data = env.storage()
            .persistent()
            .get::<_, NFTMarketData>(&NFTDataKey::LeaseInstance(token_id))
            .unwrap_or_else(|| {
                NFTMarketData {
                    token_id,
                    lease_id: 0,
                    current_price: price,
                    last_sale_price: price,
                    last_sale_timestamp: env.ledger().timestamp(),
                    price_history: Vec::new(env),
                    yield_history: Vec::new(env),
                    total_volume: 0,
                    sale_count: 0,
                }
            });
        
        // Update sale data
        market_data.last_sale_price = price;
        market_data.last_sale_timestamp = env.ledger().timestamp();
        market_data.total_volume += price;
        market_data.sale_count += 1;
        
        // Add price point to history
        let price_point = PricePoint {
            price,
            timestamp: env.ledger().timestamp(),
            volume: price,
        };
        market_data.price_history.push_back(price_point);
        
        // Store updated market data
        env.storage()
            .persistent()
            .set(&NFTDataKey::LeaseInstance(token_id), &market_data);
        
        Ok(())
    }
    
    fn update_market_data_on_price_update(env: &Env, token_id: u128, new_price: i128) -> Result<(), DEXError> {
        let mut market_data = env.storage()
            .persistent()
            .get::<_, NFTMarketData>(&NFTDataKey::LeaseInstance(token_id))
            .unwrap_or_else(|| {
                NFTMarketData {
                    token_id,
                    lease_id: 0,
                    current_price: new_price,
                    last_sale_price: 0,
                    last_sale_timestamp: 0,
                    price_history: Vec::new(env),
                    yield_history: Vec::new(env),
                    total_volume: 0,
                    sale_count: 0,
                }
            });
        
        // Update current price
        market_data.current_price = new_price;
        
        // Add price point to history
        let price_point = PricePoint {
            price: new_price,
            timestamp: env.ledger().timestamp(),
            volume: 0, // No volume on price update
        };
        market_data.price_history.push_back(price_point);
        
        // Store updated market data
        env.storage()
            .persistent()
            .set(&NFTDataKey::LeaseInstance(token_id), &market_data);
        
        Ok(())
    }
}

/// Market statistics structure
#[derive(Debug, Clone)]
pub struct MarketStatistics {
    pub total_listings: u32,
    pub active_listings: u32,
    pub total_volume_24h: i128,
    pub average_price: i128,
    pub highest_price: i128,
    pub lowest_price: i128,
    pub total_yield_distributed: i128,
    pub last_updated: u64,
}

impl MarketStatistics {
    pub fn new() -> Self {
        Self {
            total_listings: 0,
            active_listings: 0,
            total_volume_24h: 0,
            average_price: 0,
            highest_price: 0,
            lowest_price: i128::MAX,
            total_yield_distributed: 0,
            last_updated: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_nft_listing() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let admin = TestAddress::generate(&env);
        let dex_contract = TestAddress::generate(&env);
        let token_contract = TestAddress::generate(&env);
        
        // Configure DEX
        DEXIntegrationManager::configure_dex(
            env.clone(),
            admin.clone(),
            dex_contract,
            token_contract,
            250, // 2.5% fee
            100,  // min price
            100000, // max price
        ).unwrap();
        
        // Create NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 1, lessor.clone()).unwrap();
        
        // Release lock for listing
        LessorRightsNFT::release_nft_lock(env.clone(), 1).unwrap();
        
        // List NFT
        let listing_result = DEXIntegrationManager::list_nft_on_dex(
            env.clone(),
            token_id,
            lessor.clone(),
            5000,
            Some(30), // 30 days
        );
        assert!(listing_result.is_ok());
        
        // Verify listing exists
        let listing = DEXIntegrationManager::get_listing(env.clone(), token_id).unwrap();
        assert_eq!(listing.token_id, token_id);
        assert_eq!(listing.seller, lessor);
        assert_eq!(listing.price, 5000);
        assert!(listing.active);
    }

    #[test]
    fn test_nft_sale() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        let admin = TestAddress::generate(&env);
        let dex_contract = TestAddress::generate(&env);
        let token_contract = TestAddress::generate(&env);
        
        // Configure DEX
        DEXIntegrationManager::configure_dex(
            env.clone(),
            admin.clone(),
            dex_contract,
            token_contract,
            250,
            100,
            100000,
        ).unwrap();
        
        // Create and list NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 2, lessor.clone()).unwrap();
        LessorRightsNFT::release_nft_lock(env.clone(), 2).unwrap();
        
        DEXIntegrationManager::list_nft_on_dex(
            env.clone(),
            token_id,
            lessor.clone(),
            3000,
            Some(30),
        ).unwrap();
        
        // Buy NFT
        let buy_result = DEXIntegrationManager::buy_nft_from_dex(
            env.clone(),
            token_id,
            buyer.clone(),
            4000, // max price
        );
        assert!(buy_result.is_ok());
        
        // Verify NFT ownership changed
        let current_holder = LessorRightsNFT::get_current_holder(env.clone(), 2).unwrap();
        assert_eq!(current_holder, buyer);
        
        // Verify listing removed
        let listing_result = DEXIntegrationManager::get_listing(env.clone(), token_id);
        assert_eq!(listing_result, Err(DEXError::NFTNotFound));
    }

    #[test]
    fn test_market_data_tracking() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let buyer = TestAddress::generate(&env);
        let admin = TestAddress::generate(&env);
        let dex_contract = TestAddress::generate(&env);
        let token_contract = TestAddress::generate(&env);
        
        // Configure DEX
        DEXIntegrationManager::configure_dex(
            env.clone(),
            admin.clone(),
            dex_contract,
            token_contract,
            250,
            100,
            100000,
        ).unwrap();
        
        // Create and sell NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 3, lessor.clone()).unwrap();
        LessorRightsNFT::release_nft_lock(env.clone(), 3).unwrap();
        
        DEXIntegrationManager::list_nft_on_dex(
            env.clone(),
            token_id,
            lessor.clone(),
            2000,
            Some(30),
        ).unwrap();
        
        DEXIntegrationManager::buy_nft_from_dex(
            env.clone(),
            token_id,
            buyer.clone(),
            2500,
        ).unwrap();
        
        // Check market data
        let market_data = DEXIntegrationManager::get_market_data(env.clone(), token_id).unwrap();
        assert_eq!(market_data.token_id, token_id);
        assert_eq!(market_data.last_sale_price, 2000);
        assert_eq!(market_data.total_volume, 2000);
        assert_eq!(market_data.sale_count, 1);
        assert!(market_data.price_history.len() > 0);
    }

    #[test]
    fn test_price_update() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let admin = TestAddress::generate(&env);
        let dex_contract = TestAddress::generate(&env);
        let token_contract = TestAddress::generate(&env);
        
        // Configure DEX
        DEXIntegrationManager::configure_dex(
            env.clone(),
            admin.clone(),
            dex_contract,
            token_contract,
            250,
            100,
            100000,
        ).unwrap();
        
        // Create and list NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 4, lessor.clone()).unwrap();
        LessorRightsNFT::release_nft_lock(env.clone(), 4).unwrap();
        
        DEXIntegrationManager::list_nft_on_dex(
            env.clone(),
            token_id,
            lessor.clone(),
            1500,
            Some(30),
        ).unwrap();
        
        // Update price
        let update_result = DEXIntegrationManager::update_listing_price(
            env.clone(),
            token_id,
            lessor.clone(),
            1800,
        );
        assert!(update_result.is_ok());
        
        // Verify price updated
        let listing = DEXIntegrationManager::get_listing(env.clone(), token_id).unwrap();
        assert_eq!(listing.price, 1800);
    }

    #[test]
    fn test_delisting() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        let admin = TestAddress::generate(&env);
        let dex_contract = TestAddress::generate(&env);
        let token_contract = TestAddress::generate(&env);
        
        // Configure DEX
        DEXIntegrationManager::configure_dex(
            env.clone(),
            admin.clone(),
            dex_contract,
            token_contract,
            250,
            100,
            100000,
        ).unwrap();
        
        // Create and list NFT
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 5, lessor.clone()).unwrap();
        LessorRightsNFT::release_nft_lock(env.clone(), 5).unwrap();
        
        DEXIntegrationManager::list_nft_on_dex(
            env.clone(),
            token_id,
            lessor.clone(),
            1200,
            Some(30),
        ).unwrap();
        
        // Delist NFT
        let delist_result = DEXIntegrationManager::delist_nft(env.clone(), token_id, lessor.clone());
        assert!(delist_result.is_ok());
        
        // Verify listing removed
        let listing_result = DEXIntegrationManager::get_listing(env.clone(), token_id);
        assert_eq!(listing_result, Err(DEXError::NFTNotFound));
    }

    #[test]
    fn test_market_statistics() {
        let env = Env::default();
        let stats = DEXIntegrationManager::get_market_statistics(env.clone());
        
        assert_eq!(stats.total_listings, 0);
        assert_eq!(stats.active_listings, 0);
        assert_eq!(stats.total_volume_24h, 0);
        assert_eq!(stats.average_price, 0);
        assert_eq!(stats.highest_price, 0);
        assert_eq!(stats.lowest_price, i128::MAX);
    }

    #[test]
    fn test_yield_projection() {
        let env = Env::default();
        let lessor = TestAddress::generate(&env);
        
        // Create NFT with known terms
        let token_id = LessorRightsNFT::mint_lessor_rights_token(env.clone(), 6, lessor.clone()).unwrap();
        
        let metadata = LessorRightsNFT::get_nft_metadata(env.clone(), token_id).unwrap();
        assert_eq!(metadata.monthly_rent, 1000); // Based on test lease creation
    }
}
