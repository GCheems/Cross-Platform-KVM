// Error handling and recovery mechanisms
// Implements requirements 9.1, 9.2, 9.3, 9.4, 9.5

use crate::{Result, KvmError};
use crate::network::{ConnectionManager, ConnectionEvent};
use crate::state::{StateManager, NotificationService, NotificationEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use log::{error, warn, info};

/// Error log entry
#[derive(Debug, Clone)]
pub struct ErrorLogEntry {
    pub timestamp: u64,
    pub error_type: ErrorType,
    pub device_id: Option<String>,
    pub message: String,
    pub details: Option<String>,
}

/// Error types for categorization
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    NetworkInterruption,
    ConnectionFailure,
    DeviceDisconnected,
    LatencyWarning,
    AuthorizationFailure,
    InputError,
    ClipboardError,
    ConfigurationError,
    Unknown,
}

impl ErrorLogEntry {
    /// Create a new error log entry
    pub fn new(error_type: ErrorType, device_id: Option<String>, message: String) -> Self {
        Self {
            timestamp: current_timestamp_ms(),
            error_type,
            device_id,
            message,
            details: None,
        }
    }
    
    /// Add details to the error log entry
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

/// Recovery action to take
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Return control to local device
    ReturnControlToLocal,
    /// Attempt to reconnect to device
    AttemptReconnect(String),
    /// Transfer control to another device
    TransferControl(String),
    /// Log warning and continue
    LogWarning,
    /// No action needed
    None,
}

/// Trait for error recovery service
#[async_trait::async_trait]
pub trait ErrorRecoveryService: Send + Sync {
    /// Handle a network interruption
    async fn handle_network_interruption(&self, device_id: &str) -> Result<RecoveryAction>;
    
    /// Handle a device disconnection
    async fn handle_device_disconnection(&self, device_id: &str) -> Result<RecoveryAction>;
    
    /// Handle high network latency
    async fn handle_high_latency(&self, device_id: &str, latency_ms: u64) -> Result<RecoveryAction>;
    
    /// Log an error with details
    async fn log_error(&self, entry: ErrorLogEntry) -> Result<()>;
    
    /// Get error log entries
    async fn get_error_log(&self) -> Vec<ErrorLogEntry>;
    
    /// Clear error log
    async fn clear_error_log(&self) -> Result<()>;
}

/// Default implementation of error recovery service
pub struct DefaultErrorRecoveryService {
    /// Connection manager for reconnection attempts
    connection_manager: Arc<dyn ConnectionManager>,
    
    /// State manager for control transfer
    state_manager: Arc<dyn StateManager>,
    
    /// Notification service for user alerts
    notification_service: Arc<dyn NotificationService>,
    
    /// Local device ID
    local_device_id: String,
    
    /// Error log storage
    error_log: Arc<RwLock<Vec<ErrorLogEntry>>>,
    
    /// Maximum error log size
    max_log_size: usize,
    
    /// Reconnection attempts tracking (device_id -> (attempt_count, last_attempt_time))
    reconnection_attempts: Arc<RwLock<HashMap<String, (u32, Instant)>>>,
    
    /// Maximum reconnection attempts before giving up
    max_reconnection_attempts: u32,
}

impl DefaultErrorRecoveryService {
    /// Create a new error recovery service
    pub fn new(
        connection_manager: Arc<dyn ConnectionManager>,
        state_manager: Arc<dyn StateManager>,
        notification_service: Arc<dyn NotificationService>,
        local_device_id: String,
    ) -> Self {
        Self {
            connection_manager,
            state_manager,
            notification_service,
            local_device_id,
            error_log: Arc::new(RwLock::new(Vec::new())),
            max_log_size: 1000,
            reconnection_attempts: Arc::new(RwLock::new(HashMap::new())),
            max_reconnection_attempts: 3,
        }
    }
    
    /// Start monitoring connection events
    pub async fn start_monitoring(&self, mut connection_events: mpsc::Receiver<ConnectionEvent>) {
        let service = Arc::new(self.clone_for_monitoring());
        
        tokio::spawn(async move {
            while let Some(event) = connection_events.recv().await {
                match event {
                    ConnectionEvent::Disconnected(device_id) => {
                        info!("Device {} disconnected, handling...", device_id);
                        let _ = service.handle_device_disconnection(&device_id).await;
                    }
                    ConnectionEvent::Error(device_id, error_msg) => {
                        error!("Connection error for device {}: {}", device_id, error_msg);
                        let entry = ErrorLogEntry::new(
                            ErrorType::ConnectionFailure,
                            Some(device_id.clone()),
                            error_msg.clone(),
                        );
                        let _ = service.log_error(entry).await;
                        let _ = service.handle_network_interruption(&device_id).await;
                    }
                    ConnectionEvent::LatencyWarning(device_id, latency_ms) => {
                        warn!("High latency detected for device {}: {}ms", device_id, latency_ms);
                        let _ = service.handle_high_latency(&device_id, latency_ms).await;
                    }
                    ConnectionEvent::Reconnecting(device_id) => {
                        info!("Attempting to reconnect to device {}...", device_id);
                    }
                    ConnectionEvent::Reconnected(device_id) => {
                        info!("Successfully reconnected to device {}", device_id);
                        // Clear reconnection attempts
                        let mut attempts = service.reconnection_attempts.write().await;
                        attempts.remove(&device_id);
                    }
                    _ => {}
                }
            }
        });
    }
    
    /// Clone for monitoring task
    fn clone_for_monitoring(&self) -> Self {
        Self {
            connection_manager: self.connection_manager.clone(),
            state_manager: self.state_manager.clone(),
            notification_service: self.notification_service.clone(),
            local_device_id: self.local_device_id.clone(),
            error_log: self.error_log.clone(),
            max_log_size: self.max_log_size,
            reconnection_attempts: self.reconnection_attempts.clone(),
            max_reconnection_attempts: self.max_reconnection_attempts,
        }
    }
}

#[async_trait::async_trait]
impl ErrorRecoveryService for DefaultErrorRecoveryService {
    async fn handle_network_interruption(&self, device_id: &str) -> Result<RecoveryAction> {
        // Log the interruption
        let entry = ErrorLogEntry::new(
            ErrorType::NetworkInterruption,
            Some(device_id.to_string()),
            format!("Network interruption detected for device {}", device_id),
        );
        self.log_error(entry).await?;
        
        // Check if this device is currently active
        let active_device = self.state_manager.get_active_device();
        
        if active_device.as_deref() == Some(device_id) {
            // Return control to local device (Requirement 9.1)
            info!("Returning control to local device due to network interruption");
            self.state_manager.set_active_device(Some(self.local_device_id.clone())).await?;
            
            // Send notification
            let notification = NotificationEvent::Error {
                message: format!("Network connection to {} lost. Control returned to local device.", device_id),
                timestamp: current_timestamp_ms(),
            };
            self.notification_service.notify(notification).await?;
            
            // Attempt automatic reconnection (Requirement 9.2)
            let device_id_clone = device_id.to_string();
            let service = Arc::new(self.clone_for_monitoring());
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = service.attempt_reconnection(&device_id_clone).await;
            });
            
            Ok(RecoveryAction::ReturnControlToLocal)
        } else {
            // Not the active device, just attempt reconnection
            self.attempt_reconnection(device_id).await?;
            Ok(RecoveryAction::AttemptReconnect(device_id.to_string()))
        }
    }
    
    async fn handle_device_disconnection(&self, device_id: &str) -> Result<RecoveryAction> {
        // Log the disconnection
        let entry = ErrorLogEntry::new(
            ErrorType::DeviceDisconnected,
            Some(device_id.to_string()),
            format!("Device {} disconnected", device_id),
        );
        self.log_error(entry).await?;
        
        // Check if this is the active device
        let active_device = self.state_manager.get_active_device();
        
        if active_device.as_deref() == Some(device_id) {
            // Transfer control to local device (Requirement 9.3)
            info!("Active device {} disconnected, transferring control to local device", device_id);
            self.state_manager.set_active_device(Some(self.local_device_id.clone())).await?;
            
            // Send notification
            let notification = NotificationEvent::Error {
                message: format!("Device {} disconnected. Control transferred to local device.", device_id),
                timestamp: current_timestamp_ms(),
            };
            self.notification_service.notify(notification).await?;
            
            Ok(RecoveryAction::TransferControl(self.local_device_id.clone()))
        } else {
            // Not the active device, just log it
            Ok(RecoveryAction::LogWarning)
        }
    }
    
    async fn handle_high_latency(&self, device_id: &str, latency_ms: u64) -> Result<RecoveryAction> {
        // Log the latency warning (Requirement 9.4)
        let entry = ErrorLogEntry::new(
            ErrorType::LatencyWarning,
            Some(device_id.to_string()),
            format!("High network latency detected: {}ms", latency_ms),
        ).with_details(format!("Latency exceeds 100ms threshold (actual: {}ms)", latency_ms));
        
        self.log_error(entry).await?;
        
        // Send warning notification
        let notification = NotificationEvent::Warning {
            message: format!("High network latency to device {}: {}ms", device_id, latency_ms),
            timestamp: current_timestamp_ms(),
        };
        self.notification_service.notify(notification).await?;
        
        warn!("High latency to device {}: {}ms (threshold: 100ms)", device_id, latency_ms);
        
        Ok(RecoveryAction::LogWarning)
    }
    
    async fn log_error(&self, entry: ErrorLogEntry) -> Result<()> {
        // Log to system logger (Requirement 9.5)
        match entry.error_type {
            ErrorType::NetworkInterruption => {
                error!("[{}] Network Interruption - Device: {:?}, Message: {}, Details: {:?}",
                    entry.timestamp, entry.device_id, entry.message, entry.details);
            }
            ErrorType::ConnectionFailure => {
                error!("[{}] Connection Failure - Device: {:?}, Message: {}, Details: {:?}",
                    entry.timestamp, entry.device_id, entry.message, entry.details);
            }
            ErrorType::DeviceDisconnected => {
                warn!("[{}] Device Disconnected - Device: {:?}, Message: {}, Details: {:?}",
                    entry.timestamp, entry.device_id, entry.message, entry.details);
            }
            ErrorType::LatencyWarning => {
                warn!("[{}] Latency Warning - Device: {:?}, Message: {}, Details: {:?}",
                    entry.timestamp, entry.device_id, entry.message, entry.details);
            }
            _ => {
                error!("[{}] Error - Type: {:?}, Device: {:?}, Message: {}, Details: {:?}",
                    entry.timestamp, entry.error_type, entry.device_id, entry.message, entry.details);
            }
        }
        
        // Store in error log
        let mut log = self.error_log.write().await;
        log.push(entry);
        
        // Trim log if it exceeds max size
        if log.len() > self.max_log_size {
            let excess = log.len() - self.max_log_size;
            log.drain(0..excess);
        }
        
        Ok(())
    }
    
    async fn get_error_log(&self) -> Vec<ErrorLogEntry> {
        let log = self.error_log.read().await;
        log.clone()
    }
    
    async fn clear_error_log(&self) -> Result<()> {
        let mut log = self.error_log.write().await;
        log.clear();
        Ok(())
    }
}

impl DefaultErrorRecoveryService {
    /// Attempt to reconnect to a device
    async fn attempt_reconnection(&self, device_id: &str) -> Result<()> {
        // Check reconnection attempts
        let mut attempts = self.reconnection_attempts.write().await;
        let (attempt_count, last_attempt) = attempts.get(device_id)
            .map(|(count, time)| (*count, *time))
            .unwrap_or((0, Instant::now() - Duration::from_secs(60)));
        
        // Don't retry too frequently (wait at least 2 seconds between attempts)
        if last_attempt.elapsed() < Duration::from_secs(2) {
            return Ok(());
        }
        
        // Check if we've exceeded max attempts
        if attempt_count >= self.max_reconnection_attempts {
            warn!("Max reconnection attempts reached for device {}", device_id);
            return Err(KvmError::Connection(format!(
                "Max reconnection attempts ({}) reached for device {}",
                self.max_reconnection_attempts, device_id
            )));
        }
        
        // Update attempt count
        attempts.insert(device_id.to_string(), (attempt_count + 1, Instant::now()));
        drop(attempts);
        
        info!("Attempting reconnection to device {} (attempt {}/{})",
            device_id, attempt_count + 1, self.max_reconnection_attempts);
        
        // Attempt to reconnect
        match self.connection_manager.connect(device_id).await {
            Ok(_) => {
                info!("Successfully reconnected to device {}", device_id);
                
                // Clear reconnection attempts
                let mut attempts = self.reconnection_attempts.write().await;
                attempts.remove(device_id);
                
                // Send success notification
                let notification = NotificationEvent::ConnectionStatus {
                    device_id: device_id.to_string(),
                    connected: true,
                    timestamp: current_timestamp_ms(),
                };
                self.notification_service.notify(notification).await?;
                
                Ok(())
            }
            Err(e) => {
                warn!("Reconnection attempt failed for device {}: {}", device_id, e);
                
                // Log the failure
                let entry = ErrorLogEntry::new(
                    ErrorType::ConnectionFailure,
                    Some(device_id.to_string()),
                    format!("Reconnection attempt {} failed", attempt_count + 1),
                ).with_details(format!("Error: {}", e));
                self.log_error(entry).await?;
                
                Err(e)
            }
        }
    }
}

/// Get current timestamp in milliseconds since UNIX epoch
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_log_entry_creation() {
        let entry = ErrorLogEntry::new(
            ErrorType::NetworkInterruption,
            Some("device1".to_string()),
            "Test error".to_string(),
        );
        
        assert_eq!(entry.error_type, ErrorType::NetworkInterruption);
        assert_eq!(entry.device_id, Some("device1".to_string()));
        assert_eq!(entry.message, "Test error");
        assert!(entry.timestamp > 0);
        assert!(entry.details.is_none());
    }
    
    #[test]
    fn test_error_log_entry_with_details() {
        let entry = ErrorLogEntry::new(
            ErrorType::LatencyWarning,
            Some("device2".to_string()),
            "High latency".to_string(),
        ).with_details("Latency: 150ms".to_string());
        
        assert_eq!(entry.details, Some("Latency: 150ms".to_string()));
    }
}
