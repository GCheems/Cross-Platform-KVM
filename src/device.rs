use crate::Result;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tokio::sync::mpsc::Receiver;

/// Operating system type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsType {
    Windows,
    MacOS,
}

/// Device status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Online,
    Offline,
    Connecting,
}

/// Information about a discovered device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip_address: IpAddr,
    pub port: u16,
    pub os_type: OsType,
    pub public_key: Vec<u8>,
}

/// Complete device model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub os_type: OsType,
    pub ip_address: IpAddr,
    pub port: u16,
    pub public_key: Vec<u8>,
    pub screen_resolution: (u32, u32),
    pub status: DeviceStatus,
}

/// Device discovery events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    DeviceFound(DeviceInfo),
    DeviceLost(String), // device_id
}

/// Trait for device discovery service
/// Responsible for discovering other KVM instances on the local network
#[async_trait::async_trait]
pub trait DeviceDiscovery: Send + Sync {
    /// Start broadcasting this device's presence on the network
    async fn start_broadcasting(&self) -> Result<()>;

    /// Stop broadcasting
    async fn stop_broadcasting(&self) -> Result<()>;

    /// Scan the local network for other devices
    async fn scan_devices(&self) -> Result<Vec<DeviceInfo>>;

    /// Subscribe to device discovery events
    fn subscribe(&self) -> Receiver<DiscoveryEvent>;
}
