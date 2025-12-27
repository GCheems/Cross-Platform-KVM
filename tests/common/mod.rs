// Common test utilities and helpers
use cross_platform_kvm::*;

/// Helper function to create a test device info
pub fn create_test_device_info(id: &str) -> device::DeviceInfo {
    use std::net::IpAddr;
    
    device::DeviceInfo {
        id: id.to_string(),
        name: format!("Test Device {}", id),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)),
        port: 5000,
        os_type: device::OsType::MacOS,
        public_key: vec![1, 2, 3, 4],
    }
}

/// Helper function to create a test device
pub fn create_test_device(id: &str) -> device::Device {
    use std::net::IpAddr;
    
    device::Device {
        id: id.to_string(),
        name: format!("Test Device {}", id),
        os_type: device::OsType::MacOS,
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)),
        port: 5000,
        public_key: vec![1, 2, 3, 4],
        screen_resolution: (1920, 1080),
        status: device::DeviceStatus::Online,
    }
}
