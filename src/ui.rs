use crate::device::{DeviceDiscovery, DiscoveryEvent};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::mpsc;

/// UI state that tracks devices and their status
#[derive(Clone)]
pub struct UiState {
    devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    discovery: Arc<dyn DeviceDiscovery>,
    event_tx: Arc<Mutex<Option<mpsc::Sender<UiEvent>>>>,
    active_device: Arc<RwLock<Option<String>>>,
    tray_update_tx: Arc<Mutex<mpsc::Sender<()>>>,
    tray_update_rx: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

/// Device information for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip_address: String,
    pub os_type: String,
    pub status: String,
    pub is_connected: bool,
}

/// Events sent from backend to UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UiEvent {
    DeviceDiscovered { device: DeviceInfo },
    DeviceLost { device_id: String },
    DeviceConnected { device_id: String },
    DeviceDisconnected { device_id: String },
    StatusUpdate { device_id: String, status: String },
    DeviceSwitched { device_id: String, device_name: String },
    ConnectionError { device_id: String, error: String },
    NetworkWarning { message: String },
    EdgeHighlight { edge: String },
}

impl UiState {
    pub fn new(discovery: Arc<dyn DeviceDiscovery>) -> Self {
        let (tray_tx, tray_rx) = mpsc::channel(100);
        
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            discovery,
            event_tx: Arc::new(Mutex::new(None)),
            active_device: Arc::new(RwLock::new(None)),
            tray_update_tx: Arc::new(Mutex::new(tray_tx)),
            tray_update_rx: Arc::new(Mutex::new(Some(tray_rx))),
        }
    }

    /// Initialize the UI state and start listening for discovery events
    pub async fn initialize(&self) -> Result<()> {
        // Start device discovery
        self.discovery.start_broadcasting().await?;
        
        // Spawn task to handle discovery events
        let devices = self.devices.clone();
        let event_tx = self.event_tx.clone();
        let tray_update_tx = self.tray_update_tx.clone();
        let mut discovery_rx = self.discovery.subscribe();
        
        tokio::spawn(async move {
            while let Some(event) = discovery_rx.recv().await {
                match event {
                    DiscoveryEvent::DeviceFound(device) => {
                        let device_info = DeviceInfo {
                            id: device.id.clone(),
                            name: device.name.clone(),
                            ip_address: device.ip_address.to_string(),
                            os_type: format!("{:?}", device.os_type),
                            status: "Online".to_string(),
                            is_connected: false,
                        };
                        
                        devices.write().await.insert(device.id.clone(), device_info.clone());
                        
                        // Notify UI
                        if let Some(tx) = event_tx.lock().await.as_ref() {
                            let _ = tx.send(UiEvent::DeviceDiscovered { device: device_info }).await;
                        }
                        
                        // Notify tray to update
                        let _ = tray_update_tx.lock().await.send(()).await;
                    }
                    DiscoveryEvent::DeviceLost(device_id) => {
                        devices.write().await.remove(&device_id);
                        
                        // Notify UI
                        if let Some(tx) = event_tx.lock().await.as_ref() {
                            let _ = tx.send(UiEvent::DeviceLost { device_id }).await;
                        }
                        
                        // Notify tray to update
                        let _ = tray_update_tx.lock().await.send(()).await;
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Get all discovered devices
    pub async fn get_devices(&self) -> Vec<DeviceInfo> {
        self.devices.read().await.values().cloned().collect()
    }

    /// Refresh device list by rescanning
    pub async fn refresh_devices(&self) -> Result<Vec<DeviceInfo>> {
        let devices = self.discovery.scan_devices().await?;
        
        let mut device_map = self.devices.write().await;
        device_map.clear();
        
        let device_infos: Vec<DeviceInfo> = devices
            .into_iter()
            .map(|device| {
                let info = DeviceInfo {
                    id: device.id.clone(),
                    name: device.name.clone(),
                    ip_address: device.ip_address.to_string(),
                    os_type: format!("{:?}", device.os_type),
                    status: "Online".to_string(),
                    is_connected: false,
                };
                device_map.insert(device.id.clone(), info.clone());
                info
            })
            .collect();
        
        // Notify tray to update
        let _ = self.tray_update_tx.lock().await.send(()).await;
        
        Ok(device_infos)
    }

    /// Connect to a device
    pub async fn connect_device(&self, device_id: String) -> Result<()> {
        // Update device status
        if let Some(device) = self.devices.write().await.get_mut(&device_id) {
            device.is_connected = true;
            device.status = "Connected".to_string();
            
            // Notify UI
            if let Some(tx) = self.event_tx.lock().await.as_ref() {
                let _ = tx.send(UiEvent::DeviceConnected { device_id }).await;
            }
            
            // Notify tray to update
            let _ = self.tray_update_tx.lock().await.send(()).await;
        }
        
        Ok(())
    }

    /// Disconnect from a device
    pub async fn disconnect_device(&self, device_id: String) -> Result<()> {
        // Update device status
        if let Some(device) = self.devices.write().await.get_mut(&device_id) {
            device.is_connected = false;
            device.status = "Online".to_string();
            
            // Notify UI
            if let Some(tx) = self.event_tx.lock().await.as_ref() {
                let _ = tx.send(UiEvent::DeviceDisconnected { device_id }).await;
            }
            
            // Notify tray to update
            let _ = self.tray_update_tx.lock().await.send(()).await;
        }
        
        Ok(())
    }

    /// Set the event channel for UI updates
    pub async fn set_event_channel(&self, tx: mpsc::Sender<UiEvent>) {
        *self.event_tx.lock().await = Some(tx);
    }
    
    /// Get the currently active device
    pub async fn get_active_device(&self) -> Option<DeviceInfo> {
        let active_id = self.active_device.read().await;
        if let Some(id) = active_id.as_ref() {
            self.devices.read().await.get(id).cloned()
        } else {
            None
        }
    }
    
    /// Switch to a specific device
    pub async fn switch_to_device(&self, device_id: String) -> Result<()> {
        // Verify device exists and is connected
        let device_name = {
            let devices = self.devices.read().await;
            if let Some(device) = devices.get(&device_id) {
                if !device.is_connected {
                    return Err(crate::error::KvmError::InvalidState(
                        format!("Device {} is not connected", device_id)
                    ));
                }
                device.name.clone()
            } else {
                return Err(crate::error::KvmError::InvalidState(
                    format!("Device {} not found", device_id)
                ));
            }
        };
        
        // Update active device
        {
            let mut active = self.active_device.write().await;
            *active = Some(device_id.clone());
        }
        
        // Notify UI with device name
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(UiEvent::DeviceSwitched { 
                device_id: device_id.clone(),
                device_name 
            }).await;
        }
        
        // Notify tray to update
        let _ = self.tray_update_tx.lock().await.send(()).await;
        
        Ok(())
    }
    
    /// Get tray state information
    pub async fn get_tray_state(&self) -> TrayState {
        let active_device = {
            let active_id = self.active_device.read().await;
            if let Some(id) = active_id.as_ref() {
                self.devices.read().await.get(id).map(|d| d.name.clone())
            } else {
                None
            }
        };
        
        let connected_devices: Vec<DeviceInfo> = self.devices.read().await
            .values()
            .filter(|d| d.is_connected)
            .cloned()
            .collect();
        
        TrayState {
            active_device,
            connected_devices,
        }
    }
    
    /// Subscribe to tray update notifications
    pub fn subscribe_tray_updates(&self) -> mpsc::Receiver<()> {
        let mut rx_lock = futures::executor::block_on(self.tray_update_rx.lock());
        if let Some(rx) = rx_lock.take() {
            rx
        } else {
            // Create a new receiver
            let (_, rx) = mpsc::channel(100);
            rx
        }
    }
    
    /// Send a connection error notification
    pub async fn notify_connection_error(&self, device_id: String, error: String) {
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(UiEvent::ConnectionError { device_id, error }).await;
        }
    }
    
    /// Send a network warning notification
    pub async fn notify_network_warning(&self, message: String) {
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(UiEvent::NetworkWarning { message }).await;
        }
    }
    
    /// Send an edge highlight notification
    pub async fn notify_edge_highlight(&self, edge: String) {
        if let Some(tx) = self.event_tx.lock().await.as_ref() {
            let _ = tx.send(UiEvent::EdgeHighlight { edge }).await;
        }
    }
    
    /// Get local device information
    pub async fn get_local_device_info(&self) -> Result<DeviceInfo> {
        // Get hostname
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        
        // Get OS type
        #[cfg(target_os = "windows")]
        let os_type = "Windows".to_string();
        
        #[cfg(target_os = "macos")]
        let os_type = "MacOS".to_string();
        
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let os_type = "Unknown".to_string();
        
        // Generate a local device ID (could be based on MAC address or other unique identifier)
        let device_id = format!("local-{}", hostname);
        
        Ok(DeviceInfo {
            id: device_id,
            name: hostname,
            ip_address: "127.0.0.1".to_string(),
            os_type,
            status: "Local".to_string(),
            is_connected: true,
        })
    }
}

/// Tray state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayState {
    pub active_device: Option<String>,
    pub connected_devices: Vec<DeviceInfo>,
}
