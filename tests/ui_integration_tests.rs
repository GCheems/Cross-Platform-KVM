use cross_platform_kvm::{
    config::{Config, DeviceLayout, DevicePosition, EdgeSwitchingConfig, FileConfigManager, Hotkey, Preferences},
    device::{DeviceDiscovery, DiscoveryEvent, OsType},
    discovery::MockDeviceDiscovery,
    input::Modifiers,
    ui::{DeviceInfo, UiState},
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tempfile::TempDir;

/// Test device discovery and connection flow
#[tokio::test]
async fn test_device_discovery_and_connection_flow() {
    // Create mock discovery service
    let discovery = Arc::new(MockDeviceDiscovery::new());
    
    // Create UI state
    let ui_state = Arc::new(UiState::new(discovery.clone()));
    
    // Initialize UI state
    ui_state.initialize().await.expect("Failed to initialize UI state");
    
    // Initially, no devices should be discovered
    let devices = ui_state.get_devices().await;
    assert_eq!(devices.len(), 0, "Should start with no devices");
    
    // Simulate device discovery by adding a device to mock discovery
    discovery.add_mock_device(
        "device1".to_string(),
        "Test Device 1".to_string(),
        "192.168.1.100".parse().unwrap(),
        5000,
        OsType::Windows,
    ).await;
    
    // Wait a bit for the event to propagate
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Refresh devices
    let devices = ui_state.refresh_devices().await.expect("Failed to refresh devices");
    assert_eq!(devices.len(), 1, "Should have one device after discovery");
    assert_eq!(devices[0].name, "Test Device 1");
    assert_eq!(devices[0].os_type, "Windows");
    assert!(!devices[0].is_connected, "Device should not be connected initially");
    
    // Connect to the device
    ui_state.connect_device("device1".to_string()).await.expect("Failed to connect to device");
    
    // Verify device is now connected
    let devices = ui_state.get_devices().await;
    let device = devices.iter().find(|d| d.id == "device1").expect("Device not found");
    assert!(device.is_connected, "Device should be connected");
    assert_eq!(device.status, "Connected");
    
    // Disconnect from the device
    ui_state.disconnect_device("device1".to_string()).await.expect("Failed to disconnect from device");
    
    // Verify device is disconnected
    let devices = ui_state.get_devices().await;
    let device = devices.iter().find(|d| d.id == "device1").expect("Device not found");
    assert!(!device.is_connected, "Device should be disconnected");
    assert_eq!(device.status, "Online");
}

/// Test layout configuration save and load
#[tokio::test]
async fn test_layout_configuration_save_and_load() {
    // Create temporary directory for config
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");
    
    // Create config manager
    let config_manager = FileConfigManager::new(config_path.clone());
    
    // Create a layout configuration
    let mut layout = DeviceLayout {
        devices: HashMap::new(),
    };
    
    layout.devices.insert(
        "device1".to_string(),
        DevicePosition {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
            edge_switching: EdgeSwitchingConfig {
                left_enabled: true,
                right_enabled: false,
                top_enabled: true,
                bottom_enabled: true,
            },
        },
    );
    
    layout.devices.insert(
        "device2".to_string(),
        DevicePosition {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
            edge_switching: EdgeSwitchingConfig::default(),
        },
    );
    
    // Create and save config
    let config = Config {
        device_layout: layout.clone(),
        hotkeys: HashMap::new(),
        security: Default::default(),
        preferences: Preferences::default(),
    };
    
    config_manager.save(&config).await.expect("Failed to save config");
    
    // Load config
    let loaded_config = config_manager.load().await.expect("Failed to load config");
    
    // Verify layout was saved and loaded correctly
    assert_eq!(loaded_config.device_layout.devices.len(), 2);
    
    let device1_pos = loaded_config.device_layout.devices.get("device1").expect("Device1 not found");
    assert_eq!(device1_pos.x, 1920);
    assert_eq!(device1_pos.y, 0);
    assert_eq!(device1_pos.width, 1920);
    assert_eq!(device1_pos.height, 1080);
    assert!(device1_pos.edge_switching.left_enabled);
    assert!(!device1_pos.edge_switching.right_enabled);
    
    let device2_pos = loaded_config.device_layout.devices.get("device2").expect("Device2 not found");
    assert_eq!(device2_pos.x, -1920);
    assert_eq!(device2_pos.y, 0);
}

/// Test hotkey configuration
#[tokio::test]
async fn test_hotkey_configuration() {
    // Create temporary directory for config
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");
    
    // Create config manager
    let config_manager = FileConfigManager::new(config_path.clone());
    
    // Load initial config
    config_manager.load().await.expect("Failed to load config");
    
    // Add a hotkey
    let hotkey1 = Hotkey {
        key_code: 0x31, // '1' key
        modifiers: Modifiers {
            shift: false,
            ctrl: true,
            alt: true,
            meta: false,
        },
        device_id: "device1".to_string(),
        enabled: true,
    };
    
    config_manager.update_hotkey("hotkey1".to_string(), hotkey1.clone())
        .await
        .expect("Failed to add hotkey");
    
    // Verify hotkey was added
    let config = config_manager.get_config().await;
    assert_eq!(config.hotkeys.len(), 1);
    assert!(config.hotkeys.contains_key("hotkey1"));
    
    let saved_hotkey = config.hotkeys.get("hotkey1").unwrap();
    assert_eq!(saved_hotkey.key_code, 0x31);
    assert!(saved_hotkey.modifiers.ctrl);
    assert!(saved_hotkey.modifiers.alt);
    assert_eq!(saved_hotkey.device_id, "device1");
    
    // Add another hotkey
    let hotkey2 = Hotkey {
        key_code: 0x32, // '2' key
        modifiers: Modifiers {
            shift: false,
            ctrl: true,
            alt: true,
            meta: false,
        },
        device_id: "device2".to_string(),
        enabled: true,
    };
    
    config_manager.update_hotkey("hotkey2".to_string(), hotkey2)
        .await
        .expect("Failed to add second hotkey");
    
    // Verify both hotkeys exist
    let config = config_manager.get_config().await;
    assert_eq!(config.hotkeys.len(), 2);
    
    // Remove a hotkey
    config_manager.remove_hotkey("hotkey1")
        .await
        .expect("Failed to remove hotkey");
    
    // Verify hotkey was removed
    let config = config_manager.get_config().await;
    assert_eq!(config.hotkeys.len(), 1);
    assert!(!config.hotkeys.contains_key("hotkey1"));
    assert!(config.hotkeys.contains_key("hotkey2"));
}

/// Test device switching flow
#[tokio::test]
async fn test_device_switching_flow() {
    // Create mock discovery service
    let discovery = Arc::new(MockDeviceDiscovery::new());
    
    // Create UI state
    let ui_state = Arc::new(UiState::new(discovery.clone()));
    
    // Initialize UI state
    ui_state.initialize().await.expect("Failed to initialize UI state");
    
    // Add and connect devices
    discovery.add_mock_device(
        "device1".to_string(),
        "Device 1".to_string(),
        "192.168.1.100".parse().unwrap(),
        5000,
        OsType::Windows,
    ).await;
    
    discovery.add_mock_device(
        "device2".to_string(),
        "Device 2".to_string(),
        "192.168.1.101".parse().unwrap(),
        5000,
        OsType::MacOS,
    ).await;
    
    // Refresh and connect devices
    ui_state.refresh_devices().await.expect("Failed to refresh devices");
    ui_state.connect_device("device1".to_string()).await.expect("Failed to connect device1");
    ui_state.connect_device("device2".to_string()).await.expect("Failed to connect device2");
    
    // Initially, no device should be active
    let active = ui_state.get_active_device().await;
    assert!(active.is_none(), "No device should be active initially");
    
    // Switch to device1
    ui_state.switch_to_device("device1".to_string()).await.expect("Failed to switch to device1");
    
    // Verify device1 is active
    let active = ui_state.get_active_device().await;
    assert!(active.is_some(), "Device should be active");
    assert_eq!(active.unwrap().id, "device1");
    
    // Switch to device2
    ui_state.switch_to_device("device2".to_string()).await.expect("Failed to switch to device2");
    
    // Verify device2 is active
    let active = ui_state.get_active_device().await;
    assert!(active.is_some(), "Device should be active");
    assert_eq!(active.unwrap().id, "device2");
    
    // Try to switch to a non-connected device (should fail)
    discovery.add_mock_device(
        "device3".to_string(),
        "Device 3".to_string(),
        "192.168.1.102".parse().unwrap(),
        5000,
        OsType::Windows,
    ).await;
    
    ui_state.refresh_devices().await.expect("Failed to refresh devices");
    
    let result = ui_state.switch_to_device("device3".to_string()).await;
    assert!(result.is_err(), "Should not be able to switch to non-connected device");
}

/// Test onboarding completion flag
#[tokio::test]
async fn test_onboarding_completion() {
    // Create temporary directory for config
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");
    
    // Create config manager
    let config_manager = FileConfigManager::new(config_path.clone());
    
    // Load initial config
    config_manager.load().await.expect("Failed to load config");
    
    // Initially, onboarding should not be completed
    let config = config_manager.get_config().await;
    assert!(!config.preferences.onboarding_completed, "Onboarding should not be completed initially");
    
    // Mark onboarding as completed
    config_manager.set_onboarding_completed(true)
        .await
        .expect("Failed to set onboarding completed");
    
    // Verify onboarding is marked as completed
    let config = config_manager.get_config().await;
    assert!(config.preferences.onboarding_completed, "Onboarding should be completed");
    
    // Reload config from disk to verify persistence
    let config_manager2 = FileConfigManager::new(config_path.clone());
    let loaded_config = config_manager2.load().await.expect("Failed to load config");
    assert!(loaded_config.preferences.onboarding_completed, "Onboarding completion should persist");
}

/// Test edge switching configuration
#[tokio::test]
async fn test_edge_switching_configuration() {
    // Create temporary directory for config
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");
    
    // Create config manager
    let config_manager = FileConfigManager::new(config_path.clone());
    
    // Load initial config
    config_manager.load().await.expect("Failed to load config");
    
    // Update edge switching for a device
    let edge_config = EdgeSwitchingConfig {
        left_enabled: true,
        right_enabled: false,
        top_enabled: false,
        bottom_enabled: true,
    };
    
    config_manager.update_edge_switching("device1".to_string(), edge_config.clone())
        .await
        .expect("Failed to update edge switching");
    
    // Verify edge switching was updated
    let config = config_manager.get_config().await;
    let device_pos = config.device_layout.devices.get("device1").expect("Device not found");
    
    assert!(device_pos.edge_switching.left_enabled);
    assert!(!device_pos.edge_switching.right_enabled);
    assert!(!device_pos.edge_switching.top_enabled);
    assert!(device_pos.edge_switching.bottom_enabled);
}

/// Test local device info retrieval
#[tokio::test]
async fn test_local_device_info() {
    // Create mock discovery service
    let discovery = Arc::new(MockDeviceDiscovery::new());
    
    // Create UI state
    let ui_state = Arc::new(UiState::new(discovery.clone()));
    
    // Get local device info
    let local_device = ui_state.get_local_device_info().await.expect("Failed to get local device info");
    
    // Verify local device info
    assert!(!local_device.id.is_empty(), "Device ID should not be empty");
    assert!(!local_device.name.is_empty(), "Device name should not be empty");
    assert_eq!(local_device.status, "Local");
    assert!(local_device.is_connected, "Local device should always be connected");
    
    // Verify OS type is set correctly
    #[cfg(target_os = "windows")]
    assert_eq!(local_device.os_type, "Windows");
    
    #[cfg(target_os = "macos")]
    assert_eq!(local_device.os_type, "MacOS");
}
