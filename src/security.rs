// Security module for certificate generation and device authorization
// Handles TLS certificate management and device authentication

use crate::{Result, KvmError};
use ring::rand::SystemRandom;
use ring::signature::{self, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Device certificate and key pair
pub struct DeviceCertificate {
    pub certificate: CertificateDer<'static>,
    pub private_key: PrivateKeyDer<'static>,
    pub public_key: Vec<u8>,
}

/// Authorization manager for device connections
pub struct AuthorizationManager {
    /// Map of device_id -> public_key for authorized devices
    authorized_devices: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl AuthorizationManager {
    /// Create a new authorization manager
    pub fn new() -> Self {
        Self {
            authorized_devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Authorize a device by storing its public key
    pub async fn authorize_device(&self, device_id: String, public_key: Vec<u8>) -> Result<()> {
        let mut devices = self.authorized_devices.write().await;
        devices.insert(device_id, public_key);
        Ok(())
    }

    /// Check if a device is authorized
    pub async fn is_authorized(&self, device_id: &str, public_key: &[u8]) -> bool {
        let devices = self.authorized_devices.read().await;
        if let Some(stored_key) = devices.get(device_id) {
            stored_key == public_key
        } else {
            false
        }
    }

    /// Revoke authorization for a device
    pub async fn revoke_device(&self, device_id: &str) -> Result<()> {
        let mut devices = self.authorized_devices.write().await;
        devices.remove(device_id);
        Ok(())
    }

    /// Get all authorized device IDs
    pub async fn get_authorized_devices(&self) -> Vec<String> {
        let devices = self.authorized_devices.read().await;
        devices.keys().cloned().collect()
    }

    /// Get the public key for an authorized device
    pub async fn get_device_key(&self, device_id: &str) -> Option<Vec<u8>> {
        let devices = self.authorized_devices.read().await;
        devices.get(device_id).cloned()
    }
}

/// Generate a self-signed certificate for a device
pub fn generate_device_certificate(device_id: &str) -> Result<DeviceCertificate> {
    // Generate a key pair using ring
    let rng = SystemRandom::new();
    let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|e| KvmError::Security(format!("Failed to generate key pair: {:?}", e)))?;

    let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
        .map_err(|e| KvmError::Security(format!("Failed to parse key pair: {:?}", e)))?;

    let public_key = key_pair.public_key().as_ref().to_vec();

    // Create a simple self-signed certificate
    // For a production system, we would use rcgen or similar
    // For now, we'll create a minimal certificate structure
    let cert = create_self_signed_cert(device_id, &public_key)?;
    let private_key = PrivateKeyDer::Pkcs8(pkcs8_bytes.as_ref().to_vec().into());

    Ok(DeviceCertificate {
        certificate: cert,
        private_key,
        public_key,
    })
}

/// Create a self-signed certificate (simplified version)
fn create_self_signed_cert(device_id: &str, public_key: &[u8]) -> Result<CertificateDer<'static>> {
    // In a real implementation, we would use rcgen to create a proper X.509 certificate
    // For this implementation, we'll create a minimal certificate structure
    // that includes the device ID and public key
    
    // This is a simplified certificate format for demonstration
    // In production, use rcgen or similar library
    let mut cert_data = Vec::new();
    cert_data.extend_from_slice(b"CERT");
    cert_data.extend_from_slice(device_id.as_bytes());
    cert_data.extend_from_slice(public_key);
    
    Ok(CertificateDer::from(cert_data))
}

/// Extract public key from a certificate
pub fn extract_public_key(cert: &CertificateDer) -> Result<Vec<u8>> {
    // In a real implementation, we would parse the X.509 certificate
    // For this simplified version, we extract from our custom format
    let cert_data = cert.as_ref();
    
    if cert_data.len() < 4 || &cert_data[0..4] != b"CERT" {
        return Err(KvmError::Security("Invalid certificate format".to_string()));
    }
    
    // Find where the device ID ends and public key begins
    // In our format: "CERT" + device_id + public_key
    // We need to skip "CERT" and the device_id to get to the public key
    // For simplicity, we assume the public key is the last 32 bytes (Ed25519 key size)
    if cert_data.len() < 36 {
        return Err(KvmError::Security("Certificate too short".to_string()));
    }
    
    let public_key = cert_data[cert_data.len() - 32..].to_vec();
    Ok(public_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_certificate() {
        let cert = generate_device_certificate("test-device").unwrap();
        assert!(!cert.public_key.is_empty());
        assert_eq!(cert.public_key.len(), 32); // Ed25519 public key size
    }

    #[test]
    fn test_extract_public_key() {
        let cert = generate_device_certificate("test-device").unwrap();
        let extracted_key = extract_public_key(&cert.certificate).unwrap();
        assert_eq!(extracted_key, cert.public_key);
    }

    #[tokio::test]
    async fn test_authorization_manager() {
        let manager = AuthorizationManager::new();
        let device_id = "test-device".to_string();
        let public_key = vec![1, 2, 3, 4, 5];

        // Initially not authorized
        assert!(!manager.is_authorized(&device_id, &public_key).await);

        // Authorize device
        manager.authorize_device(device_id.clone(), public_key.clone()).await.unwrap();
        assert!(manager.is_authorized(&device_id, &public_key).await);

        // Wrong key should not be authorized
        let wrong_key = vec![6, 7, 8, 9, 10];
        assert!(!manager.is_authorized(&device_id, &wrong_key).await);

        // Revoke authorization
        manager.revoke_device(&device_id).await.unwrap();
        assert!(!manager.is_authorized(&device_id, &public_key).await);
    }

    #[tokio::test]
    async fn test_get_authorized_devices() {
        let manager = AuthorizationManager::new();
        
        manager.authorize_device("device1".to_string(), vec![1, 2, 3]).await.unwrap();
        manager.authorize_device("device2".to_string(), vec![4, 5, 6]).await.unwrap();

        let devices = manager.get_authorized_devices().await;
        assert_eq!(devices.len(), 2);
        assert!(devices.contains(&"device1".to_string()));
        assert!(devices.contains(&"device2".to_string()));
    }

    #[tokio::test]
    async fn test_get_device_key() {
        let manager = AuthorizationManager::new();
        let device_id = "test-device".to_string();
        let public_key = vec![1, 2, 3, 4, 5];

        manager.authorize_device(device_id.clone(), public_key.clone()).await.unwrap();

        let retrieved_key = manager.get_device_key(&device_id).await;
        assert_eq!(retrieved_key, Some(public_key));

        let missing_key = manager.get_device_key("nonexistent").await;
        assert_eq!(missing_key, None);
    }
}
