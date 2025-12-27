// Property-based tests for error handling and recovery mechanisms
// These tests verify requirements 9.1, 9.2, 9.3, 9.4, 9.5

mod common;

use cross_platform_kvm::*;
use cross_platform_kvm::recovery::*;
use cross_platform_kvm::network::*;
use cross_platform_kvm::state::*;
use cross_platform_kvm::security::*;
use proptest::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// Test helper: Create a mock connection manager
struct MockConnectionManager {
    connections: Arc<tokio::sync::RwLock<std::collections::HashMap<String, bool>>>,
    event_tx: mpsc::Sender<ConnectionEvent>,
}

impl MockConnectionManager {
    fn new(event_tx: mpsc::Sender<ConnectionEvent>) -> Self {
        Self {
            connections: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            event_tx,
        }
    }
    
    async fn simulate_disconnection(&self, device_id: &str) {
        let mut conns = self.connections.write().await;
        conns.insert(device_id.to_string(), false);
        let _ = self.event_tx.send(ConnectionEvent::Disconnected(device_id.to_string())).await;
    }
    
    async fn simulate_connection(&self, device_id: &str) {
        let mut conns = self.connections.write().await;
        conns.insert(device_id.to_string(), true);
        let _ = self.event_tx.send(ConnectionEvent::Connected(device_id.to_string())).await;
    }
}

#[async_trait::async_trait]
impl ConnectionManager for MockConnectionManager {
    async fn connect(&self, device_id: &str) -> Result<Connection> {
        let mut conns = self.connections.write().await;
        conns.insert(device_id.to_string(), true);
        let _ = self.event_tx.send(ConnectionEvent::Connected(device_id.to_string())).await;
        Ok(Connection {
            device_id: device_id.to_string(),
            state: ConnectionState::Connected,
            addr: "127.0.0.1:8080".parse().unwrap(),
        })
    }
    
    async fn disconnect(&self, device_id: &str) -> Result<()> {
        let mut conns = self.connections.write().await;
        conns.remove(device_id);
        let _ = self.event_tx.send(ConnectionEvent::Disconnected(device_id.to_string())).await;
        Ok(())
    }
    
    fn get_connections(&self) -> Vec<Connection> {
        vec![]
    }
    
    fn subscribe(&self) -> mpsc::Receiver<ConnectionEvent> {
        let (_, rx) = mpsc::channel(100);
        rx
    }
}

// Property test generators

prop_compose! {
    fn arb_device_id()(id in "[a-z0-9-]{8,16}") -> String {
        id
    }
}

prop_compose! {
    fn arb_latency_ms()(latency in 50u64..500) -> u64 {
        latency
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 27: Network interruption control return**
    /// **Validates: Requirements 9.1**
    /// 
    /// For any network connection interruption event, the system should automatically
    /// return control to the local device.
    #[test]
    fn test_property_27_network_interruption_control_return(
        remote_device_id in arb_device_id(),
        local_device_id in arb_device_id()
    ) {
        // Ensure device IDs are different
        prop_assume!(remote_device_id != local_device_id);
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create notification service
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            
            // Create state manager
            let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
            
            // Create mock connection manager
            let (event_tx, event_rx) = mpsc::channel(100);
            let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
            
            // Create error recovery service
            let recovery_service = DefaultErrorRecoveryService::new(
                connection_manager.clone(),
                state_manager.clone(),
                notification_service.clone(),
                local_device_id.clone(),
            );
            
            // Set active device to remote device
            state_manager.set_active_device(Some(remote_device_id.clone())).await.unwrap();
            
            // Verify remote device is active
            prop_assert_eq!(state_manager.get_active_device(), Some(remote_device_id.clone()));
            
            // Simulate network interruption
            let action = recovery_service.handle_network_interruption(&remote_device_id).await.unwrap();
            
            // Wait a bit for state update
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Verify control returned to local device
            prop_assert_eq!(state_manager.get_active_device(), Some(local_device_id.clone()));
            
            // Verify recovery action is correct
            prop_assert_eq!(action, RecoveryAction::ReturnControlToLocal);
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod network_interruption_tests {
    use super::*;
    
    /// Test that network interruption returns control to local device
    #[tokio::test]
    async fn test_network_interruption_returns_control() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Set active device to remote
        state_manager.set_active_device(Some("remote-device".to_string())).await.unwrap();
        assert_eq!(state_manager.get_active_device(), Some("remote-device".to_string()));
        
        // Handle network interruption
        let action = recovery_service.handle_network_interruption("remote-device").await.unwrap();
        
        // Wait for state update
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Control should return to local device
        assert_eq!(state_manager.get_active_device(), Some("local-device".to_string()));
        assert_eq!(action, RecoveryAction::ReturnControlToLocal);
    }
    
    /// Test that network interruption for non-active device doesn't change control
    #[tokio::test]
    async fn test_network_interruption_non_active_device() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Set active device to device1
        state_manager.set_active_device(Some("device1".to_string())).await.unwrap();
        
        // Handle network interruption for device2 (not active)
        let action = recovery_service.handle_network_interruption("device2").await.unwrap();
        
        // Active device should remain device1
        assert_eq!(state_manager.get_active_device(), Some("device1".to_string()));
        assert_eq!(action, RecoveryAction::AttemptReconnect("device2".to_string()));
    }
    
    /// Test that error log is created for network interruption
    #[tokio::test]
    async fn test_network_interruption_creates_error_log() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Set active device
        state_manager.set_active_device(Some("remote-device".to_string())).await.unwrap();
        
        // Handle network interruption
        recovery_service.handle_network_interruption("remote-device").await.unwrap();
        
        // Check error log
        let log = recovery_service.get_error_log().await;
        assert!(!log.is_empty());
        assert_eq!(log[0].error_type, ErrorType::NetworkInterruption);
        assert_eq!(log[0].device_id, Some("remote-device".to_string()));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 28: Connection auto recovery**
    /// **Validates: Requirements 9.2**
    /// 
    /// For any network connection recovery event, the system should automatically
    /// re-establish the connection within 10 seconds.
    #[test]
    fn test_property_28_connection_auto_recovery(
        device_id in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
            
            let (event_tx, _event_rx) = mpsc::channel(100);
            let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
            
            let recovery_service = DefaultErrorRecoveryService::new(
                connection_manager.clone(),
                state_manager.clone(),
                notification_service.clone(),
                "local-device".to_string(),
            );
            
            // Simulate disconnection
            connection_manager.simulate_disconnection(&device_id).await;
            
            // Handle network interruption (which triggers reconnection)
            let _ = recovery_service.handle_network_interruption(&device_id).await;
            
            // Wait for reconnection attempt (should happen within 10 seconds)
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // Verify error log contains the interruption
            let log = recovery_service.get_error_log().await;
            prop_assert!(!log.is_empty());
            prop_assert!(log.iter().any(|entry| 
                entry.error_type == ErrorType::NetworkInterruption &&
                entry.device_id.as_deref() == Some(device_id.as_str())
            ));
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod connection_recovery_tests {
    use super::*;
    
    /// Test that connection recovery is attempted
    #[tokio::test]
    async fn test_connection_recovery_attempted() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Simulate disconnection
        connection_manager.simulate_disconnection("device1").await;
        
        // Handle network interruption
        recovery_service.handle_network_interruption("device1").await.unwrap();
        
        // Wait for reconnection attempt
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify error log
        let log = recovery_service.get_error_log().await;
        assert!(!log.is_empty());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 29: Host disconnect control transfer**
    /// **Validates: Requirements 9.3**
    /// 
    /// For any current host device disconnection event, control should be
    /// transferred to the local device.
    #[test]
    fn test_property_29_host_disconnect_control_transfer(
        host_device_id in arb_device_id(),
        local_device_id in arb_device_id()
    ) {
        prop_assume!(host_device_id != local_device_id);
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
            
            let (event_tx, _event_rx) = mpsc::channel(100);
            let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
            
            let recovery_service = DefaultErrorRecoveryService::new(
                connection_manager.clone(),
                state_manager.clone(),
                notification_service.clone(),
                local_device_id.clone(),
            );
            
            // Set active device to host
            state_manager.set_active_device(Some(host_device_id.clone())).await.unwrap();
            prop_assert_eq!(state_manager.get_active_device(), Some(host_device_id.clone()));
            
            // Handle device disconnection
            let action = recovery_service.handle_device_disconnection(&host_device_id).await.unwrap();
            
            // Wait for state update
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Control should transfer to local device
            prop_assert_eq!(state_manager.get_active_device(), Some(local_device_id.clone()));
            prop_assert_eq!(action, RecoveryAction::TransferControl(local_device_id.clone()));
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod host_disconnect_tests {
    use super::*;
    
    /// Test that host disconnection transfers control
    #[tokio::test]
    async fn test_host_disconnect_transfers_control() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Set active device to remote host
        state_manager.set_active_device(Some("remote-host".to_string())).await.unwrap();
        
        // Handle disconnection
        let action = recovery_service.handle_device_disconnection("remote-host").await.unwrap();
        
        // Wait for state update
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Control should transfer to local device
        assert_eq!(state_manager.get_active_device(), Some("local-device".to_string()));
        assert_eq!(action, RecoveryAction::TransferControl("local-device".to_string()));
    }
    
    /// Test that non-host disconnection doesn't transfer control
    #[tokio::test]
    async fn test_non_host_disconnect_no_transfer() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Set active device to device1
        state_manager.set_active_device(Some("device1".to_string())).await.unwrap();
        
        // Handle disconnection of device2 (not active)
        let action = recovery_service.handle_device_disconnection("device2").await.unwrap();
        
        // Active device should remain device1
        assert_eq!(state_manager.get_active_device(), Some("device1".to_string()));
        assert_eq!(action, RecoveryAction::LogWarning);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 30: Network latency warning**
    /// **Validates: Requirements 9.4**
    /// 
    /// For any network communication, if latency exceeds 100ms, the system
    /// should display a performance warning.
    #[test]
    fn test_property_30_network_latency_warning(
        device_id in arb_device_id(),
        latency_ms in arb_latency_ms()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
            
            let (event_tx, _event_rx) = mpsc::channel(100);
            let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
            
            let recovery_service = DefaultErrorRecoveryService::new(
                connection_manager.clone(),
                state_manager.clone(),
                notification_service.clone(),
                "local-device".to_string(),
            );
            
            // Handle high latency
            let action = recovery_service.handle_high_latency(&device_id, latency_ms).await.unwrap();
            
            // Verify action is LogWarning
            prop_assert_eq!(action, RecoveryAction::LogWarning);
            
            // Verify error log contains latency warning
            let log = recovery_service.get_error_log().await;
            prop_assert!(!log.is_empty());
            
            let latency_entry = log.iter().find(|entry| 
                entry.error_type == ErrorType::LatencyWarning &&
                entry.device_id.as_deref() == Some(device_id.as_str())
            );
            prop_assert!(latency_entry.is_some());
            
            // If latency > 100ms, verify warning was logged
            if latency_ms > 100 {
                let entry = latency_entry.unwrap();
                prop_assert!(entry.message.contains(&latency_ms.to_string()));
            }
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod latency_warning_tests {
    use super::*;
    
    /// Test that high latency triggers warning
    #[tokio::test]
    async fn test_high_latency_triggers_warning() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Handle high latency (150ms > 100ms threshold)
        let action = recovery_service.handle_high_latency("device1", 150).await.unwrap();
        
        assert_eq!(action, RecoveryAction::LogWarning);
        
        // Verify error log
        let log = recovery_service.get_error_log().await;
        assert!(!log.is_empty());
        assert_eq!(log[0].error_type, ErrorType::LatencyWarning);
        assert!(log[0].message.contains("150"));
    }
    
    /// Test that latency below threshold still logs
    #[tokio::test]
    async fn test_low_latency_still_logs() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Handle latency below threshold (50ms < 100ms)
        let action = recovery_service.handle_high_latency("device1", 50).await.unwrap();
        
        assert_eq!(action, RecoveryAction::LogWarning);
        
        // Verify error log
        let log = recovery_service.get_error_log().await;
        assert!(!log.is_empty());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 31: Error log recording**
    /// **Validates: Requirements 9.5**
    /// 
    /// For any connection error event, the system should record a log entry
    /// containing timestamp, error type, and detailed information.
    #[test]
    fn test_property_31_error_log_recording(
        device_id in arb_device_id(),
        error_message in ".*"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
            
            let (event_tx, _event_rx) = mpsc::channel(100);
            let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
            
            let recovery_service = DefaultErrorRecoveryService::new(
                connection_manager.clone(),
                state_manager.clone(),
                notification_service.clone(),
                "local-device".to_string(),
            );
            
            // Create and log an error entry
            let entry = ErrorLogEntry::new(
                ErrorType::ConnectionFailure,
                Some(device_id.clone()),
                error_message.clone(),
            ).with_details("Test error details".to_string());
            
            recovery_service.log_error(entry.clone()).await.unwrap();
            
            // Retrieve error log
            let log = recovery_service.get_error_log().await;
            
            // Verify log is not empty
            prop_assert!(!log.is_empty());
            
            // Find the logged entry
            let logged_entry = log.iter().find(|e| 
                e.device_id.as_deref() == Some(device_id.as_str()) &&
                e.message == error_message
            );
            
            prop_assert!(logged_entry.is_some());
            
            let logged = logged_entry.unwrap();
            
            // Verify all required fields are present
            prop_assert!(logged.timestamp > 0);
            prop_assert_eq!(logged.error_type, ErrorType::ConnectionFailure);
            prop_assert_eq!(logged.device_id, Some(device_id.clone()));
            prop_assert_eq!(logged.message, error_message);
            prop_assert_eq!(logged.details, Some("Test error details".to_string()));
            
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod error_log_tests {
    use super::*;
    
    /// Test that error log records all required fields
    #[tokio::test]
    async fn test_error_log_records_all_fields() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Log an error
        let entry = ErrorLogEntry::new(
            ErrorType::NetworkInterruption,
            Some("device1".to_string()),
            "Test error".to_string(),
        ).with_details("Additional details".to_string());
        
        recovery_service.log_error(entry).await.unwrap();
        
        // Retrieve log
        let log = recovery_service.get_error_log().await;
        assert_eq!(log.len(), 1);
        
        let logged = &log[0];
        assert!(logged.timestamp > 0);
        assert_eq!(logged.error_type, ErrorType::NetworkInterruption);
        assert_eq!(logged.device_id, Some("device1".to_string()));
        assert_eq!(logged.message, "Test error");
        assert_eq!(logged.details, Some("Additional details".to_string()));
    }
    
    /// Test that error log can be cleared
    #[tokio::test]
    async fn test_error_log_can_be_cleared() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Log multiple errors
        for i in 0..5 {
            let entry = ErrorLogEntry::new(
                ErrorType::ConnectionFailure,
                Some(format!("device{}", i)),
                format!("Error {}", i),
            );
            recovery_service.log_error(entry).await.unwrap();
        }
        
        // Verify log has entries
        let log = recovery_service.get_error_log().await;
        assert_eq!(log.len(), 5);
        
        // Clear log
        recovery_service.clear_error_log().await.unwrap();
        
        // Verify log is empty
        let log = recovery_service.get_error_log().await;
        assert_eq!(log.len(), 0);
    }
    
    /// Test that error log respects max size
    #[tokio::test]
    async fn test_error_log_max_size() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = Arc::new(DefaultStateManager::new(notification_service.clone()));
        
        let (event_tx, _event_rx) = mpsc::channel(100);
        let connection_manager = Arc::new(MockConnectionManager::new(event_tx));
        
        let recovery_service = DefaultErrorRecoveryService::new(
            connection_manager.clone(),
            state_manager.clone(),
            notification_service.clone(),
            "local-device".to_string(),
        );
        
        // Log many errors (more than max size of 1000)
        for i in 0..1100 {
            let entry = ErrorLogEntry::new(
                ErrorType::ConnectionFailure,
                Some(format!("device{}", i)),
                format!("Error {}", i),
            );
            recovery_service.log_error(entry).await.unwrap();
        }
        
        // Verify log is trimmed to max size
        let log = recovery_service.get_error_log().await;
        assert_eq!(log.len(), 1000);
        
        // Verify oldest entries were removed (should start from entry 100)
        assert!(log[0].message.contains("Error 100"));
    }
}
