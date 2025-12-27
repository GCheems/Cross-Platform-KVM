use crate::device::{DeviceDiscovery, DeviceInfo, DiscoveryEvent, OsType};
use crate::{KvmError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tokio::time;

/// UDP multicast address for device discovery
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
const MULTICAST_PORT: u16 = 5353;

/// Heartbeat interval (5 seconds)
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// Device timeout (30 seconds)
const DEVICE_TIMEOUT: Duration = Duration::from_secs(30);

/// Discovery broadcast message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryMessage {
    device_id: String,
    device_name: String,
    port: u16,
    os_type: OsType,
    public_key: Vec<u8>,
    timestamp: u64,
}

/// Tracked device with last seen timestamp
#[derive(Debug, Clone)]
struct TrackedDevice {
    info: DeviceInfo,
    last_seen: Instant,
}

/// Device discovery service implementation
pub struct DiscoveryService {
    device_id: String,
    device_name: String,
    port: u16,
    os_type: OsType,
    public_key: Vec<u8>,
    devices: Arc<RwLock<HashMap<String, TrackedDevice>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
    event_rx: Arc<RwLock<Option<mpsc::Receiver<DiscoveryEvent>>>>,
    broadcast_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    listen_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    timeout_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl DiscoveryService {
    /// Create a new discovery service
    pub fn new(
        device_id: String,
        device_name: String,
        port: u16,
        os_type: OsType,
        public_key: Vec<u8>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        Self {
            device_id,
            device_name,
            port,
            os_type,
            public_key,
            devices: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
            broadcast_handle: Arc::new(RwLock::new(None)),
            listen_handle: Arc::new(RwLock::new(None)),
            timeout_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a device manually (for testing)
    #[cfg(test)]
    pub async fn add_device_for_test(&self, device: DeviceInfo, last_seen: Instant) {
        let mut devices = self.devices.write().await;
        devices.insert(
            device.id.clone(),
            TrackedDevice {
                info: device,
                last_seen,
            },
        );
    }

    /// Get device count (for testing)
    #[cfg(test)]
    pub async fn device_count(&self) -> usize {
        self.devices.read().await.len()
    }

    /// Start the listening task for incoming discovery messages
    async fn start_listening(&self) -> Result<()> {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, MULTICAST_PORT)))
            .await
            .map_err(|e| KvmError::Network(format!("Failed to bind UDP socket: {}", e)))?;

        // Join multicast group
        socket
            .join_multicast_v4(MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED)
            .map_err(|e| KvmError::Network(format!("Failed to join multicast group: {}", e)))?;

        let devices = self.devices.clone();
        let event_tx = self.event_tx.clone();
        let own_device_id = self.device_id.clone();

        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 65536];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(&buf[..len]) {
                            // Ignore our own broadcasts
                            if msg.device_id == own_device_id {
                                continue;
                            }

                            let device_info = DeviceInfo {
                                id: msg.device_id.clone(),
                                name: msg.device_name,
                                ip_address: addr.ip(),
                                port: msg.port,
                                os_type: msg.os_type,
                                public_key: msg.public_key,
                            };

                            let mut devices = devices.write().await;
                            let is_new = !devices.contains_key(&msg.device_id);

                            devices.insert(
                                msg.device_id.clone(),
                                TrackedDevice {
                                    info: device_info.clone(),
                                    last_seen: Instant::now(),
                                },
                            );

                            if is_new {
                                let _ = event_tx.send(DiscoveryEvent::DeviceFound(device_info)).await;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error receiving discovery message: {}", e);
                    }
                }
            }
        });

        *self.listen_handle.write().await = Some(handle);
        Ok(())
    }

    /// Start the timeout checker task
    async fn start_timeout_checker(&self) {
        let devices = self.devices.clone();
        let event_tx = self.event_tx.clone();

        let handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;

                let mut devices = devices.write().await;
                let now = Instant::now();
                let mut to_remove = Vec::new();

                for (device_id, tracked) in devices.iter() {
                    if now.duration_since(tracked.last_seen) > DEVICE_TIMEOUT {
                        to_remove.push(device_id.clone());
                    }
                }

                for device_id in to_remove {
                    devices.remove(&device_id);
                    let _ = event_tx.send(DiscoveryEvent::DeviceLost(device_id)).await;
                }
            }
        });

        *self.timeout_handle.write().await = Some(handle);
    }
}

#[async_trait::async_trait]
impl DeviceDiscovery for DiscoveryService {
    async fn start_broadcasting(&self) -> Result<()> {
        // Start listening for incoming messages
        self.start_listening().await?;

        // Start timeout checker
        self.start_timeout_checker().await;

        // Create broadcast socket
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| KvmError::Network(format!("Failed to create broadcast socket: {}", e)))?;

        socket
            .set_broadcast(true)
            .map_err(|e| KvmError::Network(format!("Failed to enable broadcast: {}", e)))?;

        let multicast_addr = SocketAddr::from((MULTICAST_ADDR, MULTICAST_PORT));
        let device_id = self.device_id.clone();
        let device_name = self.device_name.clone();
        let port = self.port;
        let os_type = self.os_type;
        let public_key = self.public_key.clone();

        let handle = tokio::spawn(async move {
            let mut interval = time::interval(HEARTBEAT_INTERVAL);
            loop {
                interval.tick().await;

                let message = DiscoveryMessage {
                    device_id: device_id.clone(),
                    device_name: device_name.clone(),
                    port,
                    os_type,
                    public_key: public_key.clone(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                if let Ok(json) = serde_json::to_vec(&message) {
                    if let Err(e) = socket.send_to(&json, multicast_addr).await {
                        log::error!("Failed to send discovery broadcast: {}", e);
                    }
                }
            }
        });

        *self.broadcast_handle.write().await = Some(handle);
        Ok(())
    }

    async fn stop_broadcasting(&self) -> Result<()> {
        // Stop broadcast task
        if let Some(handle) = self.broadcast_handle.write().await.take() {
            handle.abort();
        }

        // Stop listen task
        if let Some(handle) = self.listen_handle.write().await.take() {
            handle.abort();
        }

        // Stop timeout checker
        if let Some(handle) = self.timeout_handle.write().await.take() {
            handle.abort();
        }

        Ok(())
    }

    async fn scan_devices(&self) -> Result<Vec<DeviceInfo>> {
        let devices = self.devices.read().await;
        Ok(devices
            .values()
            .map(|tracked| tracked.info.clone())
            .collect())
    }

    fn subscribe(&self) -> mpsc::Receiver<DiscoveryEvent> {
        // This is a simplified implementation - in production, we'd want to support multiple subscribers
        let mut rx_lock = self.event_rx.blocking_write();
        rx_lock.take().expect("subscribe() can only be called once")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let service = DiscoveryService::new(
            "test-device".to_string(),
            "Test Device".to_string(),
            8080,
            OsType::MacOS,
            vec![1, 2, 3, 4],
        );

        assert_eq!(service.device_id, "test-device");
        assert_eq!(service.device_name, "Test Device");
        assert_eq!(service.port, 8080);
    }

    #[tokio::test]
    async fn test_scan_devices_empty() {
        let service = DiscoveryService::new(
            "test-device".to_string(),
            "Test Device".to_string(),
            8080,
            OsType::MacOS,
            vec![1, 2, 3, 4],
        );

        let devices = service.scan_devices().await.unwrap();
        assert_eq!(devices.len(), 0);
    }
}

/// Mock device discovery for testing and UI development
pub struct MockDeviceDiscovery {
    devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
    event_rx: Arc<RwLock<Option<mpsc::Receiver<DiscoveryEvent>>>>,
}

impl MockDeviceDiscovery {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }

    /// Add a mock device for testing
    pub async fn add_mock_device(&self, device: DeviceInfo) {
        self.devices.write().await.insert(device.id.clone(), device.clone());
        let _ = self.event_tx.send(DiscoveryEvent::DeviceFound(device)).await;
    }
}

#[async_trait::async_trait]
impl DeviceDiscovery for MockDeviceDiscovery {
    async fn start_broadcasting(&self) -> Result<()> {
        // Mock implementation - add some sample devices
        let sample_devices = vec![
            DeviceInfo {
                id: "device-1".to_string(),
                name: "MacBook Pro".to_string(),
                ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
                port: 8080,
                os_type: OsType::MacOS,
                public_key: vec![1, 2, 3, 4],
            },
            DeviceInfo {
                id: "device-2".to_string(),
                name: "Windows Desktop".to_string(),
                ip_address: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)),
                port: 8080,
                os_type: OsType::Windows,
                public_key: vec![5, 6, 7, 8],
            },
        ];

        for device in sample_devices {
            self.add_mock_device(device).await;
        }

        Ok(())
    }

    async fn stop_broadcasting(&self) -> Result<()> {
        Ok(())
    }

    async fn scan_devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self.devices.read().await.values().cloned().collect())
    }

    fn subscribe(&self) -> mpsc::Receiver<DiscoveryEvent> {
        let mut rx_lock = self.event_rx.blocking_write();
        rx_lock.take().expect("subscribe() can only be called once")
    }
}
