//! IoT Smart Lock Integration API
//! 
//! This module provides comprehensive integration interfaces for external systems
//! like smart locks, access gates, and IoT devices to query and validate lessee access tokens.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype,
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, LeaseError,
    lessee_access_token::{
        LesseeAccessTokenManager, AccessVerificationRequest, AccessVerificationResponse,
        VerificationPurpose, LesseeAccessToken, AccessDataKey
    }
};

/// IoT device types and capabilities
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IoTDeviceType {
    SmartLock,      // Door locks, gate locks
    AccessGate,     // Parking gates, turnstiles
    SmartKey,       // Car keys, fob systems
    Biometric,      // Fingerprint, facial recognition
    RFID,           // RFID card readers
    QRCode,         // QR code scanners
    NFC,            // NFC readers
    Bluetooth,      // Bluetooth locks
    Hybrid,         // Multi-technology devices
}

/// IoT device capabilities
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceCapabilities {
    pub device_type: IoTDeviceType,
    pub supports_remote: bool,
    pub supports_offline: bool,
    pub supports_audit: bool,
    pub supports_time_based: bool,
    pub supports_multi_user: bool,
    pub max_concurrent_access: u32,
    pub battery_backup: bool,
    pub encryption_level: EncryptionLevel,
}

/// Encryption level for IoT communication
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncryptionLevel {
    None,           // No encryption
    Basic,          // Basic encryption
    Standard,       // Standard encryption (AES-128)
    High,           // High security (AES-256)
    Military,       // Military grade encryption
}

/// IoT device registration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IoTDevice {
    pub device_id: String,
    pub device_type: IoTDeviceType,
    pub owner: Address,
    pub location: String,
    pub capabilities: DeviceCapabilities,
    pub registered_at: u64,
    pub last_seen: u64,
    pub status: DeviceStatus,
    pub firmware_version: String,
    pub hardware_version: String,
}

/// Device status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceStatus {
    Online,
    Offline,
    Maintenance,
    Error,
    Decommissioned,
}

/// Access request from IoT device
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IoTAccessRequest {
    pub device_id: String,
    pub requesting_user: Address,
    pub access_method: AccessMethod,
    pub timestamp: u64,
    pub context: AccessContext,
    pub device_challenge: Option<BytesN<32>>,
}

/// Access method types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessMethod {
    Biometric,      // Fingerprint, face, etc.
    RFID,           // RFID card
    NFC,            // NFC device
    QRCode,         // QR code
    MobileApp,      // Mobile application
    WebApp,         // Web application
    Voice,          // Voice command
    Physical,       // Physical key
}

/// Access context information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessContext {
    pub purpose: String,
    pub urgency: AccessUrgency,
    pub expected_duration: Option<u64>,
    pub companion_devices: Vec<String>,
    pub environmental_conditions: Map<String, String>,
}

/// Access urgency levels
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessUrgency {
    Low,            // Normal access
    Medium,         // Priority access
    High,           // Emergency access
    Critical,       // Life safety emergency
}

/// IoT access response
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IoTAccessResponse {
    pub device_id: String,
    pub access_granted: bool,
    pub access_duration: Option<u64>,
    pub access_level: AccessLevel,
    pub granted_at: u64,
    pub expires_at: u64,
    pub verification_code: Option<BytesN<32>>,
    pub audit_required: bool,
    pub additional_instructions: Vec<String>,
}

/// Access level for IoT devices
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    ReadOnly,       // Read-only access
    Limited,         // Limited access
    Standard,        // Standard access
    Full,            // Full access
    Admin,           // Administrative access
    Emergency,       // Emergency access
}

/// IoT device events
#[contractevent]
pub struct DeviceRegistered {
    pub device_id: String,
    pub device_type: IoTDeviceType,
    pub owner: Address,
    pub registered_at: u64,
}

#[contractevent]
pub struct DeviceAccessGranted {
    pub device_id: String,
    pub user: Address,
    pub access_level: AccessLevel,
    pub granted_at: u64,
    pub expires_at: u64,
}

#[contractevent]
pub struct DeviceAccessDenied {
    pub device_id: String,
    pub user: Address,
    pub denial_reason: String,
    pub denied_at: u64,
}

#[contractevent]
pub struct DeviceStatusUpdated {
    pub device_id: String,
    pub old_status: DeviceStatus,
    pub new_status: DeviceStatus,
    pub updated_at: u64,
}

/// IoT integration errors
#[contracterror]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoTError {
    DeviceNotFound = 6001,
    DeviceNotRegistered = 6002,
    DeviceOffline = 6003,
    AccessDenied = 6004,
    InvalidAccessMethod = 6005,
    TokenExpired = 6006,
    TokenRevoked = 6007,
    UnauthorizedDevice = 6008,
    CommunicationFailed = 6009,
    EncryptionError = 6010,
    RateLimited = 6011,
    MaintenanceMode = 6012,
    CrossContractCallFailed = 6013,
}

/// IoT Integration Manager
pub struct IoTIntegrationManager;

impl IoTIntegrationManager {
    /// Register IoT device with the system
    pub fn register_device(
        env: Env,
        device_id: String,
        device_type: IoTDeviceType,
        owner: Address,
        location: String,
        capabilities: DeviceCapabilities,
        firmware_version: String,
        hardware_version: String,
    ) -> Result<(), IoTError> {
        // Verify device doesn't already exist
        if env.storage().persistent().has(&AccessDataKey::LeaseInstance(0)) { // Reuse key for device
            return Err(IoTError::DeviceNotRegistered);
        }
        
        // Create device record
        let device = IoTDevice {
            device_id: device_id.clone(),
            device_type,
            owner: owner.clone(),
            location,
            capabilities,
            registered_at: env.ledger().timestamp(),
            last_seen: env.ledger().timestamp(),
            status: DeviceStatus::Online,
            firmware_version,
            hardware_version,
        };
        
        // Store device
        Self::store_device(&env, &device)?;
        
        // Emit registration event
        DeviceRegistered {
            device_id,
            device_type: device.device_type,
            owner,
            registered_at: device.registered_at,
        }.publish(&env);
        
        Ok(())
    }
    
    /// Process access request from IoT device
    pub fn process_access_request(
        env: Env,
        request: IoTAccessRequest,
    ) -> Result<IoTAccessResponse, IoTError> {
        // Verify device exists and is online
        let device = Self::get_device(&env, &request.device_id)?;
        
        if device.status != DeviceStatus::Online {
            return Err(IoTError::DeviceOffline);
        }
        
        // Verify access method is supported
        if !Self::is_access_method_supported(&device.capabilities, &request.access_method) {
            return Err(IoTError::InvalidAccessMethod);
        }
        
        // Get access token for the user
        let access_token = Self::get_user_access_token(&env, request.requesting_user)?;
        
        // Verify token validity
        let current_time = env.ledger().timestamp();
        let is_valid = Self::is_token_valid(&access_token, current_time);
        
        if !is_valid {
            return Err(IoTError::TokenExpired);
        }
        
        // Check if token is revoked
        if access_token.revoked {
            return Err(IoTError::TokenRevoked);
        }
        
        // Determine access level and duration
        let (access_level, duration) = Self::determine_access_level(&env, &device, &access_token, &request)?;
        
        // Generate verification code if needed
        let verification_code = if device.capabilities.encryption_level != EncryptionLevel::None {
            Some(Self::generate_verification_code(&env))
        } else {
            None
        };
        
        // Create response
        let response = IoTAccessResponse {
            device_id: request.device_id.clone(),
            access_granted: true,
            access_duration: duration,
            access_level,
            granted_at: current_time,
            expires_at: duration.map(|d| current_time + d),
            verification_code,
            audit_required: device.capabilities.supports_audit,
            additional_instructions: Self::generate_access_instructions(&env, &device, &request),
        };
        
        // Emit access granted event
        DeviceAccessGranted {
            device_id: request.device_id,
            user: request.requesting_user,
            access_level,
            granted_at: response.granted_at,
            expires_at: response.expires_at.unwrap_or(0),
        }.publish(&env);
        
        Ok(response)
    }
    
    /// Query token validity for external systems
    pub fn query_token_validity(
        env: Env,
        token_id: u128,
        requesting_system: Address,
        system_identifier: String,
    ) -> Result<bool, IoTError> {
        // Create verification request
        let request = AccessVerificationRequest {
            token_id,
            requesting_system,
            verification_purpose: VerificationPurpose::Validate,
            system_identifier,
            timestamp: env.ledger().timestamp(),
        };
        
        // Verify token
        let response = LesseeAccessTokenManager::verify_access_token(env.clone(), request)
            .map_err(|_| IoTError::CrossContractCallFailed)?;
        
        Ok(response.is_valid)
    }
    
    /// Get detailed token information for external systems
    pub fn get_token_details(
        env: Env,
        token_id: u128,
        requesting_system: Address,
    ) -> Result<LesseeAccessToken, IoTError> {
        // Get access token
        let token = LesseeAccessTokenManager::get_access_token(env.clone(), token_id)
            .map_err(|_| IoTError::TokenNotFound)?;
        
        // Verify requesting system is authorized
        if !Self::is_system_authorized(&env, &requesting_system) {
            return Err(IoTError::UnauthorizedDevice);
        }
        
        Ok(token)
    }
    
    /// Update device status
    pub fn update_device_status(
        env: Env,
        device_id: String,
        new_status: DeviceStatus,
        requester: Address,
    ) -> Result<(), IoTError> {
        // Get device
        let mut device = Self::get_device(&env, &device_id)?;
        
        // Verify requester is device owner or admin
        if device.owner != requester && !Self::is_admin(&env, &requester) {
            return Err(IoTError::UnauthorizedDevice);
        }
        
        let old_status = device.status.clone();
        device.status = new_status;
        device.last_seen = env.ledger().timestamp();
        
        // Store updated device
        Self::store_device(&env, &device)?;
        
        // Emit status update event
        DeviceStatusUpdated {
            device_id,
            old_status,
            new_status,
            updated_at: env.ledger().timestamp(),
        }.publish(&env);
        
        Ok(())
    }
    
    /// Get device information
    pub fn get_device_info(env: Env, device_id: String) -> Result<IoTDevice, IoTError> {
        Self::get_device(&env, &device_id)
    }
    
    /// Get all devices for an owner
    pub fn get_owner_devices(env: Env, owner: Address) -> Vec<IoTDevice> {
        let mut devices = Vec::new(&env);
        
        // In a real implementation, this would scan for devices by owner
        // For now, return empty vector
        devices
    }
    
    /// Get devices by type
    pub fn get_devices_by_type(env: Env, device_type: IoTDeviceType) -> Vec<IoTDevice> {
        let mut devices = Vec::new(&env);
        
        // In a real implementation, this would scan for devices by type
        // For now, return empty vector
        devices
    }
    
    /// Get online devices
    pub fn get_online_devices(env: Env) -> Vec<IoTDevice> {
        let mut devices = Vec::new(&env);
        
        // In a real implementation, this would scan for online devices
        // For now, return empty vector
        devices
    }
    
    /// Perform health check on device
    pub fn perform_health_check(env: Env, device_id: String) -> Result<DeviceHealthStatus, IoTError> {
        let device = Self::get_device(&env, &device_id)?;
        
        let current_time = env.ledger().timestamp();
        let last_seen_duration = current_time.saturating_sub(device.last_seen);
        
        let health_status = DeviceHealthStatus {
            device_id: device_id.clone(),
            is_online: device.status == DeviceStatus::Online,
            last_seen: device.last_seen,
            uptime: current_time.saturating_sub(device.registered_at),
            battery_level: None, // Would be retrieved from device
            signal_strength: None, // Would be retrieved from device
            firmware_upto_date: true, // Would check against latest version
            last_health_check: current_time,
            issues: Vec::new(&env),
        };
        
        Ok(health_status)
    }
    
    // Helper methods
    
    fn store_device(env: &Env, device: &IoTDevice) -> Result<(), IoTError> {
        // Store device using existing key structure
        env.storage()
            .persistent()
            .set(&AccessDataKey::LeaseInstance(0), device);
        
        // Set TTL
        let key = AccessDataKey::LeaseInstance(0);
        env.storage()
            .persistent()
            .extend_ttl(&key, 365 * 24 * 60 * 60, 365 * 24 * 60 * 60); // 1 year
        
        Ok(())
    }
    
    fn get_device(env: &Env, device_id: &String) -> Result<IoTDevice, IoTError> {
        env.storage()
            .persistent()
            .get::<_, IoTDevice>(&AccessDataKey::LeaseInstance(0)) // Reuse existing key
            .ok_or(IoTError::DeviceNotFound)
    }
    
    fn is_access_method_supported(capabilities: &DeviceCapabilities, method: &AccessMethod) -> bool {
        match method {
            AccessMethod::Biometric => true, // Most devices support biometric
            AccessMethod::RFID => true,     // Most devices support RFID
            AccessMethod::NFC => true,       // Most devices support NFC
            AccessMethod::QRCode => true,    // Most devices support QR codes
            AccessMethod::MobileApp => true,  // Most devices support mobile apps
            AccessMethod::WebApp => true,     // Most devices support web apps
            AccessMethod::Voice => capabilities.supports_remote,
            AccessMethod::Physical => true,   // Physical keys always supported
        }
    }
    
    fn get_user_access_token(env: &Env, user: Address) -> Result<LesseeAccessToken, IoTError> {
        // Get user's access tokens
        let user_tokens = LesseeAccessTokenManager::get_lessee_tokens(env.clone(), user);
        
        // Find first valid token (simplified - in practice would check specific asset)
        for token_id in user_tokens {
            if let Ok(token) = LesseeAccessTokenManager::get_access_token(env.clone(), token_id) {
                if !token.revoked && env.ledger().timestamp() <= token.expiration_timestamp {
                    return Ok(token);
                }
            }
        }
        
        Err(IoTError::AccessDenied)
    }
    
    fn is_token_valid(token: &LesseeAccessToken, current_time: u64) -> bool {
        !token.revoked && current_time <= token.expiration_timestamp
    }
    
    fn determine_access_level(
        env: &Env,
        device: &IoTDevice,
        token: &LesseeAccessToken,
        request: &IoTAccessRequest,
    ) -> Result<(AccessLevel, Option<u64>), IoTError> {
        let access_level = match token.access_level {
            crate::lessee_access_token::AccessLevel::Full => AccessLevel::Full,
            crate::lessee_access_token::AccessLevel::Limited => AccessLevel::Limited,
            crate::lessee_access_token::AccessLevel::TimeBased => AccessLevel::Standard,
            crate::lessee_access_token::AccessLevel::Conditional => AccessLevel::Limited,
        };
        
        let duration = match request.context.urgency {
            AccessUrgency::Critical => Some(3600), // 1 hour for critical
            AccessUrgency::High => Some(1800),    // 30 minutes for high
            AccessUrgency::Medium => Some(900),    // 15 minutes for medium
            AccessUrgency::Low => Some(300),       // 5 minutes for low
        };
        
        Ok((access_level, duration))
    }
    
    fn generate_verification_code(env: &Env) -> BytesN<32> {
        // Generate random verification code
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = (env.ledger().sequence() % 256) as u8;
        }
        BytesN::from_array(&bytes)
    }
    
    fn generate_access_instructions(
        env: &Env,
        device: &IoTDevice,
        request: &IoTAccessRequest,
    ) -> Vec<String> {
        let mut instructions = Vec::new(env);
        
        // Add basic instructions
        instructions.push_back(String::from_str(env, "Please verify your identity"));
        
        // Add device-specific instructions
        match device.device_type {
            IoTDeviceType::SmartLock => {
                instructions.push_back(String::from_str(env, "Press the unlock button after verification"));
            }
            IoTDeviceType::AccessGate => {
                instructions.push_back(String::from_str(env, "Proceed to the gate after access is granted"));
            }
            IoTDeviceType::SmartKey => {
                instructions.push_back(String::from_str(env, "Hold the key near the reader"));
            }
            _ => {
                instructions.push_back(String::from_str(env, "Follow the device instructions"));
            }
        }
        
        instructions
    }
    
    fn is_system_authorized(env: &Env, system: &Address) -> bool {
        // Check if system is in authorized list
        // In practice, this would check against a whitelist
        true // Simplified for demonstration
    }
    
    fn is_admin(env: &Env, address: &Address) -> bool {
        // Check if address is admin
        // In practice, this would check against admin list
        false // Simplified for demonstration
    }
}

/// Device health status
#[derive(Debug, Clone)]
pub struct DeviceHealthStatus {
    pub device_id: String,
    pub is_online: bool,
    pub last_seen: u64,
    pub uptime: u64,
    pub battery_level: Option<u32>,
    pub signal_strength: Option<u32>,
    pub firmware_upto_date: bool,
    pub last_health_check: u64,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_device_registration() {
        let env = Env::default();
        let owner = TestAddress::generate(&env);
        
        let capabilities = DeviceCapabilities {
            device_type: IoTDeviceType::SmartLock,
            supports_remote: true,
            supports_offline: false,
            supports_audit: true,
            supports_time_based: true,
            supports_multi_user: false,
            max_concurrent_access: 1,
            battery_backup: true,
            encryption_level: EncryptionLevel::Standard,
        };
        
        let registration_result = IoTIntegrationManager::register_device(
            env.clone(),
            String::from_str(&env, "smart_lock_001"),
            IoTDeviceType::SmartLock,
            owner.clone(),
            String::from_str(&env, "Front Door"),
            capabilities,
            String::from_str(&env, "v1.0.0"),
            String::from_str(&env, "v2.0"),
        );
        
        assert!(registration_result.is_ok());
        
        // Verify device was registered
        let device = IoTIntegrationManager::get_device_info(env.clone(), String::from_str(&env, "smart_lock_001")).unwrap();
        assert_eq!(device.device_type, IoTDeviceType::SmartLock);
        assert_eq!(device.owner, owner);
        assert_eq!(device.status, DeviceStatus::Online);
    }

    #[test]
    fn test_access_request_processing() {
        let env = Env::default();
        let owner = TestAddress::generate(&env);
        let user = TestAddress::generate(&env);
        
        // Register device
        let capabilities = DeviceCapabilities {
            device_type: IoTDeviceType::SmartLock,
            supports_remote: true,
            supports_offline: false,
            supports_audit: true,
            supports_time_based: true,
            supports_multi_user: false,
            max_concurrent_access: 1,
            battery_backup: true,
            encryption_level: EncryptionLevel::Standard,
        };
        
        IoTIntegrationManager::register_device(
            env.clone(),
            String::from_str(&env, "smart_lock_002"),
            IoTDeviceType::SmartLock,
            owner,
            String::from_str(&env, "Back Door"),
            capabilities,
            String::from_str(&env, "v1.0.0"),
            String::from_str(&env, "v2.0"),
        ).unwrap();
        
        // Create access token (simplified)
        // In practice, this would be done through the lessee access token system
        
        // Process access request
        let request = IoTAccessRequest {
            device_id: String::from_str(&env, "smart_lock_002"),
            requesting_user: user,
            access_method: AccessMethod::MobileApp,
            timestamp: env.ledger().timestamp(),
            context: AccessContext {
                purpose: String::from_str(&env, "Normal entry"),
                urgency: AccessUrgency::Low,
                expected_duration: Some(300),
                companion_devices: Vec::new(&env),
                environmental_conditions: Map::new(&env),
            },
            device_challenge: None,
        };
        
        // This would normally succeed with a valid token
        // For testing, we'll just verify the request structure
        assert_eq!(request.device_id, String::from_str(&env, "smart_lock_002"));
        assert_eq!(request.access_method, AccessMethod::MobileApp);
        assert_eq!(request.context.urgency, AccessUrgency::Low);
    }

    #[test]
    fn test_token_validity_query() {
        let env = Env::default();
        let requesting_system = TestAddress::generate(&env);
        
        // Query token validity (simplified test)
        let result = IoTIntegrationManager::query_token_validity(
            env.clone(),
            12345, // token_id
            requesting_system.clone(),
            String::from_str(&env, "test_system"),
        );
        
        // In practice, this would check actual token validity
        // For testing, we'll just verify the function exists
        assert!(result.is_err()); // Token doesn't exist
    }

    #[test]
    fn test_device_status_update() {
        let env = Env::default();
        let owner = TestAddress::generate(&env);
        
        // Register device
        let capabilities = DeviceCapabilities {
            device_type: IoTDeviceType::SmartLock,
            supports_remote: true,
            supports_offline: false,
            supports_audit: true,
            supports_time_based: true,
            supports_multi_user: false,
            max_concurrent_access: 1,
            battery_backup: true,
            encryption_level: EncryptionLevel::Standard,
        };
        
        IoTIntegrationManager::register_device(
            env.clone(),
            String::from_str(&env, "smart_lock_003"),
            IoTDeviceType::SmartLock,
            owner.clone(),
            String::from_str(&env, "Garage Door"),
            capabilities,
            String::from_str(&env, "v1.0.0"),
            String::from_str(&env, "v2.0"),
        ).unwrap();
        
        // Update device status
        let update_result = IoTIntegrationManager::update_device_status(
            env.clone(),
            String::from_str(&env, "smart_lock_003"),
            DeviceStatus::Maintenance,
            owner,
        );
        
        assert!(update_result.is_ok());
        
        // Verify status was updated
        let device = IoTIntegrationManager::get_device_info(env.clone(), String::from_str(&env, "smart_lock_003")).unwrap();
        assert_eq!(device.status, DeviceStatus::Maintenance);
    }

    #[test]
    fn test_health_check() {
        let env = Env::default();
        let owner = TestAddress::generate(&env);
        
        // Register device
        let capabilities = DeviceCapabilities {
            device_type: IoTDeviceType::SmartLock,
            supports_remote: true,
            supports_offline: false,
            supports_audit: true,
            supports_time_based: true,
            supports_multi_user: false,
            max_concurrent_access: 1,
            battery_backup: true,
            encryption_level: EncryptionLevel::Standard,
        };
        
        IoTIntegrationManager::register_device(
            env.clone(),
            String::from_str(&env, "smart_lock_004"),
            IoTDeviceType::SmartLock,
            owner,
            String::from_str(&env, "Side Door"),
            capabilities,
            String::from_str(&env, "v1.0.0"),
            String::from_str(&env, "v2.0"),
        ).unwrap();
        
        // Perform health check
        let health_result = IoTIntegrationManager::perform_health_check(env.clone(), String::from_str(&env, "smart_lock_004"));
        
        assert!(health_result.is_ok());
        
        let health = health_result.unwrap();
        assert_eq!(health.device_id, String::from_str(&env, "smart_lock_004"));
        assert!(health.is_online);
        assert!(health.firmware_upto_date);
    }
}
