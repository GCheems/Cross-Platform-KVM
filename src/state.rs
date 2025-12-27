use crate::{Result, KvmError};
use crate::switch::SwitchEvent;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

/// Notification event types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationEvent {
    /// Device switch notification
    DeviceSwitched {
        from_device: Option<String>,
        to_device: String,
        timestamp: u64,
    },
    /// Connection status notification
    ConnectionStatus {
        device_id: String,
        connected: bool,
        timestamp: u64,
    },
    /// Error notification
    Error {
        message: String,
        timestamp: u64,
    },
    /// Warning notification
    Warning {
        message: String,
        timestamp: u64,
    },
}

/// State indicator update event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateIndicatorUpdate {
    /// Currently active device ID
    pub active_device: Option<String>,
    /// Timestamp when the update occurred
    pub timestamp: u64,
    /// Time when the state change happened (for measuring sync delay)
    pub state_change_time: u64,
}

/// Trait for notification service
#[async_trait::async_trait]
pub trait NotificationService: Send + Sync {
    /// Send a notification
    async fn notify(&self, event: NotificationEvent) -> Result<()>;
    
    /// Subscribe to notification events
    fn subscribe(&self) -> mpsc::Receiver<NotificationEvent>;
    
    /// Check if notifications are enabled
    fn is_enabled(&self) -> bool;
    
    /// Enable or disable notifications
    async fn set_enabled(&self, enabled: bool) -> Result<()>;
}

/// Trait for state management
#[async_trait::async_trait]
pub trait StateManager: Send + Sync {
    /// Get the currently active device
    fn get_active_device(&self) -> Option<String>;
    
    /// Set the active device and broadcast state update
    async fn set_active_device(&self, device_id: Option<String>) -> Result<()>;
    
    /// Subscribe to state indicator updates
    fn subscribe_state_updates(&self) -> mpsc::Receiver<StateIndicatorUpdate>;
    
    /// Get the last state update timestamp
    fn get_last_update_time(&self) -> Option<u64>;
}

/// Default implementation of notification service
pub struct DefaultNotificationService {
    /// Whether notifications are enabled
    enabled: Arc<RwLock<bool>>,
    
    /// Notification event broadcaster
    event_tx: mpsc::Sender<NotificationEvent>,
    
    /// Notification event receiver (for subscription)
    event_rx: Arc<RwLock<Option<mpsc::Receiver<NotificationEvent>>>>,
}

impl DefaultNotificationService {
    /// Create a new notification service
    pub fn new(enabled: bool) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        Self {
            enabled: Arc::new(RwLock::new(enabled)),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }
}

#[async_trait::async_trait]
impl NotificationService for DefaultNotificationService {
    async fn notify(&self, event: NotificationEvent) -> Result<()> {
        let enabled = self.enabled.read().await;
        if !*enabled {
            return Ok(());
        }
        drop(enabled);
        
        // Send notification event
        self.event_tx.send(event).await
            .map_err(|e| KvmError::InvalidState(format!("Failed to send notification: {}", e)))?;
        
        Ok(())
    }
    
    fn subscribe(&self) -> mpsc::Receiver<NotificationEvent> {
        let mut rx_lock = futures::executor::block_on(self.event_rx.write());
        if let Some(rx) = rx_lock.take() {
            rx
        } else {
            // Create a new receiver
            let (_, rx) = mpsc::channel(100);
            rx
        }
    }
    
    fn is_enabled(&self) -> bool {
        *futures::executor::block_on(self.enabled.read())
    }
    
    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let mut enabled_lock = self.enabled.write().await;
        *enabled_lock = enabled;
        Ok(())
    }
}

/// Default implementation of state manager
pub struct DefaultStateManager {
    /// Currently active device
    active_device: Arc<RwLock<Option<String>>>,
    
    /// Last state update timestamp (milliseconds since epoch)
    last_update_time: Arc<RwLock<Option<u64>>>,
    
    /// State update broadcaster
    state_tx: mpsc::Sender<StateIndicatorUpdate>,
    
    /// State update receiver (for subscription)
    state_rx: Arc<RwLock<Option<mpsc::Receiver<StateIndicatorUpdate>>>>,
    
    /// Notification service for sending notifications
    notification_service: Arc<dyn NotificationService>,
}

impl DefaultStateManager {
    /// Create a new state manager
    pub fn new(notification_service: Arc<dyn NotificationService>) -> Self {
        let (state_tx, state_rx) = mpsc::channel(100);
        
        Self {
            active_device: Arc::new(RwLock::new(None)),
            last_update_time: Arc::new(RwLock::new(None)),
            state_tx,
            state_rx: Arc::new(RwLock::new(Some(state_rx))),
            notification_service,
        }
    }
    
    /// Start listening to switch events and update state accordingly
    pub async fn start_listening(&self, mut switch_events: mpsc::Receiver<SwitchEvent>) {
        let active_device = self.active_device.clone();
        let last_update_time = self.last_update_time.clone();
        let state_tx = self.state_tx.clone();
        let notification_service = self.notification_service.clone();
        
        tokio::spawn(async move {
            while let Some(event) = switch_events.recv().await {
                match event {
                    SwitchEvent::SwitchedTo(device_id) => {
                        let state_change_time = current_timestamp_ms();
                        
                        // Get previous device
                        let prev_device = {
                            let active = active_device.read().await;
                            active.clone()
                        };
                        
                        // Update active device
                        {
                            let mut active = active_device.write().await;
                            *active = Some(device_id.clone());
                        }
                        
                        // Update timestamp
                        let update_time = current_timestamp_ms();
                        {
                            let mut last_time = last_update_time.write().await;
                            *last_time = Some(update_time);
                        }
                        
                        // Broadcast state update
                        let update = StateIndicatorUpdate {
                            active_device: Some(device_id.clone()),
                            timestamp: update_time,
                            state_change_time,
                        };
                        let _ = state_tx.send(update).await;
                        
                        // Send notification
                        let notification = NotificationEvent::DeviceSwitched {
                            from_device: prev_device,
                            to_device: device_id,
                            timestamp: update_time,
                        };
                        let _ = notification_service.notify(notification).await;
                    }
                    SwitchEvent::SwitchFailed(device_id, error) => {
                        // Send error notification
                        let notification = NotificationEvent::Error {
                            message: format!("Failed to switch to device {}: {}", device_id, error),
                            timestamp: current_timestamp_ms(),
                        };
                        let _ = notification_service.notify(notification).await;
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl StateManager for DefaultStateManager {
    fn get_active_device(&self) -> Option<String> {
        futures::executor::block_on(self.active_device.read()).clone()
    }
    
    async fn set_active_device(&self, device_id: Option<String>) -> Result<()> {
        let state_change_time = current_timestamp_ms();
        
        // Update active device
        {
            let mut active = self.active_device.write().await;
            *active = device_id.clone();
        }
        
        // Update timestamp
        let update_time = current_timestamp_ms();
        {
            let mut last_time = self.last_update_time.write().await;
            *last_time = Some(update_time);
        }
        
        // Broadcast state update
        let update = StateIndicatorUpdate {
            active_device: device_id,
            timestamp: update_time,
            state_change_time,
        };
        self.state_tx.send(update).await
            .map_err(|e| KvmError::InvalidState(format!("Failed to send state update: {}", e)))?;
        
        Ok(())
    }
    
    fn subscribe_state_updates(&self) -> mpsc::Receiver<StateIndicatorUpdate> {
        let mut rx_lock = futures::executor::block_on(self.state_rx.write());
        if let Some(rx) = rx_lock.take() {
            rx
        } else {
            // Create a new receiver
            let (_, rx) = mpsc::channel(100);
            rx
        }
    }
    
    fn get_last_update_time(&self) -> Option<u64> {
        *futures::executor::block_on(self.last_update_time.read())
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
    
    #[tokio::test]
    async fn test_notification_service_enabled() {
        let service = DefaultNotificationService::new(true);
        assert!(service.is_enabled());
        
        let mut rx = service.subscribe();
        
        let event = NotificationEvent::DeviceSwitched {
            from_device: Some("device1".to_string()),
            to_device: "device2".to_string(),
            timestamp: current_timestamp_ms(),
        };
        
        service.notify(event.clone()).await.unwrap();
        
        let received = rx.recv().await.unwrap();
        assert_eq!(received, event);
    }
    
    #[tokio::test]
    async fn test_notification_service_disabled() {
        let service = DefaultNotificationService::new(false);
        assert!(!service.is_enabled());
        
        let mut rx = service.subscribe();
        
        let event = NotificationEvent::DeviceSwitched {
            from_device: Some("device1".to_string()),
            to_device: "device2".to_string(),
            timestamp: current_timestamp_ms(),
        };
        
        service.notify(event.clone()).await.unwrap();
        
        // Should not receive anything since notifications are disabled
        tokio::select! {
            _ = rx.recv() => panic!("Should not receive notification when disabled"),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
    
    #[tokio::test]
    async fn test_state_manager_set_active_device() {
        let notification_service = Arc::new(DefaultNotificationService::new(true));
        let state_manager = DefaultStateManager::new(notification_service);
        
        let mut rx = state_manager.subscribe_state_updates();
        
        state_manager.set_active_device(Some("device1".to_string())).await.unwrap();
        
        assert_eq!(state_manager.get_active_device(), Some("device1".to_string()));
        
        let update = rx.recv().await.unwrap();
        assert_eq!(update.active_device, Some("device1".to_string()));
        assert!(update.timestamp > 0);
    }
}
