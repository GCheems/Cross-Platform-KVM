// Property-based tests for hotkey system
// These tests verify that hotkey properties hold across all valid inputs

mod common;

use cross_platform_kvm::*;
use cross_platform_kvm::hotkey::*;
use cross_platform_kvm::config::Hotkey;
use cross_platform_kvm::input::Modifiers;
use cross_platform_kvm::switch::{DefaultSwitchController, SwitchController};
use cross_platform_kvm::device::Device;
use proptest::prelude::*;
use std::sync::Arc;
use std::time::Duration;

// Property test generators

prop_compose! {
    fn arb_modifiers()(
        shift in any::<bool>(),
        ctrl in any::<bool>(),
        alt in any::<bool>(),
        meta in any::<bool>()
    ) -> Modifiers {
        Modifiers { shift, ctrl, alt, meta }
    }
}

prop_compose! {
    fn arb_hotkey()(
        key_code in 0x41u32..0x5B, // A-Z keys
        modifiers in arb_modifiers(),
        device_id in "[a-z0-9-]{8,16}",
        enabled in any::<bool>()
    ) -> Hotkey {
        Hotkey {
            key_code,
            modifiers,
            device_id,
            enabled,
        }
    }
}

prop_compose! {
    fn arb_device_id()(id in "[a-z0-9-]{8,16}") -> String {
        id
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 18: Hotkey switch immediacy**
    /// **Validates: Requirements 6.1, 6.4**
    /// 
    /// For any configured hotkey, pressing that hotkey should immediately switch
    /// to the corresponding device and send a switch confirmation.
    #[test]
    fn test_property_18_hotkey_switch_immediacy(
        key_code in 0x41u32..0x5B, // A-Z keys
        modifiers in arb_modifiers(),
        device_id in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create switch controller
            let switch_controller = Arc::new(DefaultSwitchController::new(200));
            
            // Register a device
            let device = Device {
                id: device_id.clone(),
                name: format!("Device {}", device_id),
                os_type: device::OsType::MacOS,
                ip_address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)),
                port: 8080,
                public_key: vec![1, 2, 3],
                screen_resolution: (1920, 1080),
                status: device::DeviceStatus::Online,
            };
            switch_controller.register_device(device).await;
            
            // Create hotkey manager
            let manager = DefaultHotkeyManager::new(switch_controller.clone());
            
            // Register hotkey
            let hotkey = Hotkey {
                key_code,
                modifiers,
                device_id: device_id.clone(),
                enabled: true,
            };
            
            manager.register_hotkey("test_hotkey".to_string(), hotkey).await.unwrap();
            
            // Start listening
            manager.start_listening().await.unwrap();
            
            // Subscribe to hotkey events
            let mut event_rx = manager.subscribe();
            
            // Record time before key press
            let start_time = std::time::Instant::now();
            
            // Simulate hotkey press
            manager.handle_key_press(key_code, modifiers, true).await.unwrap();
            
            // Check that event was received immediately
            let event = tokio::time::timeout(
                Duration::from_millis(100),
                event_rx.recv()
            ).await;
            
            prop_assert!(event.is_ok(), "Hotkey event should be received immediately");
            let hotkey_event = event.unwrap().unwrap();
            
            // Verify event details
            prop_assert_eq!(hotkey_event.hotkey_id, "test_hotkey");
            prop_assert_eq!(hotkey_event.device_id, device_id);
            prop_assert_eq!(hotkey_event.key_code, key_code);
            prop_assert_eq!(hotkey_event.modifiers, modifiers);
            
            // Verify switch happened immediately (< 50ms)
            let elapsed = start_time.elapsed();
            prop_assert!(elapsed.as_millis() < 50, 
                "Switch should happen immediately, took {}ms", elapsed.as_millis());
            
            // Verify active device changed
            let active = switch_controller.get_active_device();
            prop_assert_eq!(active, Some(device_id.clone()));
            
            Ok(())
        })?;
    }
}


proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 19: Hotkey conflict validation**
    /// **Validates: Requirements 6.2**
    /// 
    /// For any hotkey that conflicts with system hotkeys, the system should
    /// reject the configuration.
    #[test]
    fn test_property_19_hotkey_conflict_validation(
        key_code in 0x41u32..0x5B,
        modifiers in arb_modifiers(),
        device_id1 in arb_device_id(),
        device_id2 in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let switch_controller = Arc::new(DefaultSwitchController::new(200));
            let manager = DefaultHotkeyManager::new(switch_controller);
            
            // Register first hotkey
            let hotkey1 = Hotkey {
                key_code,
                modifiers,
                device_id: device_id1.clone(),
                enabled: true,
            };
            
            let result1 = manager.register_hotkey("hotkey1".to_string(), hotkey1).await;
            prop_assert!(result1.is_ok(), "First hotkey registration should succeed");
            
            // Try to register conflicting hotkey (same key combination)
            let hotkey2 = Hotkey {
                key_code,
                modifiers,
                device_id: device_id2.clone(),
                enabled: true,
            };
            
            let result2 = manager.register_hotkey("hotkey2".to_string(), hotkey2).await;
            
            // If device IDs are different, this should fail due to conflict
            if device_id1 != device_id2 {
                prop_assert!(result2.is_err(), 
                    "Conflicting hotkey registration should fail");
                
                // Verify conflict detection
                let conflict = manager.check_conflict(key_code, modifiers).await;
                prop_assert!(conflict.is_some(), "Conflict should be detected");
                
                if let Some(c) = conflict {
                    prop_assert_eq!(c.existing_hotkey_id, "hotkey1");
                    prop_assert_eq!(c.existing_device_id, device_id1);
                    prop_assert_eq!(c.key_code, key_code);
                    prop_assert_eq!(c.modifiers, modifiers);
                }
            }
            
            Ok(())
        })?;
    }
}


proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 20: Hotkey uniqueness**
    /// **Validates: Requirements 6.3**
    /// 
    /// For any two different devices, they should not be assigned the same hotkey.
    #[test]
    fn test_property_20_hotkey_uniqueness(
        key_code1 in 0x41u32..0x5B,
        key_code2 in 0x41u32..0x5B,
        modifiers1 in arb_modifiers(),
        modifiers2 in arb_modifiers(),
        device_id1 in arb_device_id(),
        device_id2 in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Skip if device IDs are the same
            if device_id1 == device_id2 {
                return Ok(());
            }
            
            let switch_controller = Arc::new(DefaultSwitchController::new(200));
            let manager = DefaultHotkeyManager::new(switch_controller);
            
            // Register hotkey for device1
            let hotkey1 = Hotkey {
                key_code: key_code1,
                modifiers: modifiers1,
                device_id: device_id1.clone(),
                enabled: true,
            };
            
            manager.register_hotkey("hotkey1".to_string(), hotkey1).await.unwrap();
            
            // Try to register hotkey for device2
            let hotkey2 = Hotkey {
                key_code: key_code2,
                modifiers: modifiers2,
                device_id: device_id2.clone(),
                enabled: true,
            };
            
            let result2 = manager.register_hotkey("hotkey2".to_string(), hotkey2).await;
            
            // If key combinations are the same, registration should fail
            if key_code1 == key_code2 && modifiers1 == modifiers2 {
                prop_assert!(result2.is_err(), 
                    "Cannot assign same hotkey to different devices");
            } else {
                // Different key combinations should succeed
                prop_assert!(result2.is_ok(), 
                    "Different hotkeys for different devices should succeed");
                
                // Verify both hotkeys are registered
                let hotkeys = manager.get_hotkeys().await;
                prop_assert_eq!(hotkeys.len(), 2);
                
                // Verify each device has its own unique hotkey
                let h1 = hotkeys.get("hotkey1").unwrap();
                let h2 = hotkeys.get("hotkey2").unwrap();
                
                prop_assert_ne!((h1.key_code, h1.modifiers), (h2.key_code, h2.modifiers),
                    "Hotkeys for different devices must be unique");
            }
            
            Ok(())
        })?;
    }
}


proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Feature: cross-platform-kvm, Property 21: Hotkey disable effectiveness**
    /// **Validates: Requirements 6.5**
    /// 
    /// For any hotkey in disabled state, pressing the hotkey should not trigger
    /// a device switch.
    #[test]
    fn test_property_21_hotkey_disable_effectiveness(
        key_code in 0x41u32..0x5B,
        modifiers in arb_modifiers(),
        device_id in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let switch_controller = Arc::new(DefaultSwitchController::new(200));
            
            // Register a device
            let device = Device {
                id: device_id.clone(),
                name: format!("Device {}", device_id),
                os_type: device::OsType::MacOS,
                ip_address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)),
                port: 8080,
                public_key: vec![1, 2, 3],
                screen_resolution: (1920, 1080),
                status: device::DeviceStatus::Online,
            };
            switch_controller.register_device(device).await;
            
            let manager = DefaultHotkeyManager::new(switch_controller.clone());
            
            // Register hotkey (initially enabled)
            let hotkey = Hotkey {
                key_code,
                modifiers,
                device_id: device_id.clone(),
                enabled: true,
            };
            
            manager.register_hotkey("test_hotkey".to_string(), hotkey).await.unwrap();
            manager.start_listening().await.unwrap();
            
            // Disable the hotkey
            manager.disable_hotkey("test_hotkey").await.unwrap();
            
            // Verify hotkey is disabled
            let hotkeys = manager.get_hotkeys().await;
            let disabled_hotkey = hotkeys.get("test_hotkey").unwrap();
            prop_assert!(!disabled_hotkey.enabled, "Hotkey should be disabled");
            
            // Subscribe to events
            let mut event_rx = manager.subscribe();
            
            // Try to trigger the disabled hotkey
            manager.handle_key_press(key_code, modifiers, true).await.unwrap();
            
            // Wait a bit to see if any event is triggered
            let event = tokio::time::timeout(
                Duration::from_millis(100),
                event_rx.recv()
            ).await;
            
            // No event should be received (disabled hotkey should not trigger)
            prop_assert!(event.is_err(), "Disabled hotkey should not trigger events");
            
            // Verify active device did NOT change
            let active = switch_controller.get_active_device();
            prop_assert_ne!(active, Some(device_id.clone()), 
                "Device should not switch when hotkey is disabled");
            
            // Now enable the hotkey
            manager.enable_hotkey("test_hotkey").await.unwrap();
            
            // Verify hotkey is enabled
            let hotkeys = manager.get_hotkeys().await;
            let enabled_hotkey = hotkeys.get("test_hotkey").unwrap();
            prop_assert!(enabled_hotkey.enabled, "Hotkey should be enabled");
            
            // Subscribe to events again
            let mut event_rx2 = manager.subscribe();
            
            // Try to trigger the enabled hotkey
            manager.handle_key_press(key_code, modifiers, true).await.unwrap();
            
            // Event should be received now
            let event2 = tokio::time::timeout(
                Duration::from_millis(100),
                event_rx2.recv()
            ).await;
            
            prop_assert!(event2.is_ok(), "Enabled hotkey should trigger events");
            
            // Verify active device changed
            let active2 = switch_controller.get_active_device();
            prop_assert_eq!(active2, Some(device_id.clone()), 
                "Device should switch when hotkey is enabled");
            
            Ok(())
        })?;
    }
}


// Unit tests for hotkey functionality

#[cfg(test)]
mod hotkey_unit_tests {
    use super::*;

    /// Test that hotkey manager can be created
    #[tokio::test]
    async fn test_hotkey_manager_creation() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkeys = manager.get_hotkeys().await;
        assert_eq!(hotkeys.len(), 0, "New manager should have no hotkeys");
    }

    /// Test that hotkey can be registered and retrieved
    #[tokio::test]
    async fn test_register_and_retrieve_hotkey() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkey = Hotkey {
            key_code: 0x41, // A
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                alt: false,
                meta: false,
            },
            device_id: "device1".to_string(),
            enabled: true,
        };
        
        manager.register_hotkey("hotkey1".to_string(), hotkey.clone()).await.unwrap();
        
        let hotkeys = manager.get_hotkeys().await;
        assert_eq!(hotkeys.len(), 1);
        
        let retrieved = hotkeys.get("hotkey1").unwrap();
        assert_eq!(retrieved.key_code, hotkey.key_code);
        assert_eq!(retrieved.modifiers, hotkey.modifiers);
        assert_eq!(retrieved.device_id, hotkey.device_id);
        assert_eq!(retrieved.enabled, hotkey.enabled);
    }


    /// Test that conflicting hotkeys are rejected
    #[tokio::test]
    async fn test_conflicting_hotkeys_rejected() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkey1 = Hotkey {
            key_code: 0x41,
            modifiers: Modifiers { shift: true, ctrl: true, alt: false, meta: false },
            device_id: "device1".to_string(),
            enabled: true,
        };
        
        manager.register_hotkey("hotkey1".to_string(), hotkey1).await.unwrap();
        
        // Try to register conflicting hotkey
        let hotkey2 = Hotkey {
            key_code: 0x41,
            modifiers: Modifiers { shift: true, ctrl: true, alt: false, meta: false },
            device_id: "device2".to_string(),
            enabled: true,
        };
        
        let result = manager.register_hotkey("hotkey2".to_string(), hotkey2).await;
        assert!(result.is_err(), "Conflicting hotkey should be rejected");
    }

    /// Test that hotkey can be unregistered
    #[tokio::test]
    async fn test_unregister_hotkey() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkey = Hotkey {
            key_code: 0x41,
            modifiers: Modifiers::default(),
            device_id: "device1".to_string(),
            enabled: true,
        };
        
        manager.register_hotkey("hotkey1".to_string(), hotkey).await.unwrap();
        assert_eq!(manager.get_hotkeys().await.len(), 1);
        
        manager.unregister_hotkey("hotkey1").await.unwrap();
        assert_eq!(manager.get_hotkeys().await.len(), 0);
    }


    /// Test that multiple hotkeys can be registered for different devices
    #[tokio::test]
    async fn test_multiple_hotkeys_different_devices() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkeys = vec![
            ("hotkey1", 0x41, "device1"), // Ctrl+A -> device1
            ("hotkey2", 0x42, "device2"), // Ctrl+B -> device2
            ("hotkey3", 0x43, "device3"), // Ctrl+C -> device3
        ];
        
        for (id, key_code, device_id) in hotkeys {
            let hotkey = Hotkey {
                key_code,
                modifiers: Modifiers { shift: false, ctrl: true, alt: false, meta: false },
                device_id: device_id.to_string(),
                enabled: true,
            };
            manager.register_hotkey(id.to_string(), hotkey).await.unwrap();
        }
        
        let registered = manager.get_hotkeys().await;
        assert_eq!(registered.len(), 3);
    }

    /// Test that hotkey with empty device_id is rejected
    #[tokio::test]
    async fn test_empty_device_id_rejected() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let hotkey = Hotkey {
            key_code: 0x41,
            modifiers: Modifiers::default(),
            device_id: "".to_string(), // Empty device ID
            enabled: true,
        };
        
        let result = manager.register_hotkey("hotkey1".to_string(), hotkey).await;
        assert!(result.is_err(), "Hotkey with empty device_id should be rejected");
    }

    /// Test conflict detection
    #[tokio::test]
    async fn test_conflict_detection() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);
        
        let modifiers = Modifiers { shift: true, ctrl: true, alt: false, meta: false };
        
        // No conflict initially
        let conflict = manager.check_conflict(0x41, modifiers).await;
        assert!(conflict.is_none());
        
        // Register a hotkey
        let hotkey = Hotkey {
            key_code: 0x41,
            modifiers,
            device_id: "device1".to_string(),
            enabled: true,
        };
        manager.register_hotkey("hotkey1".to_string(), hotkey).await.unwrap();
        
        // Now there should be a conflict
        let conflict = manager.check_conflict(0x41, modifiers).await;
        assert!(conflict.is_some());
        
        let c = conflict.unwrap();
        assert_eq!(c.existing_hotkey_id, "hotkey1");
        assert_eq!(c.existing_device_id, "device1");
        assert_eq!(c.key_code, 0x41);
        assert_eq!(c.modifiers, modifiers);
    }
}
