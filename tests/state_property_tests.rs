// Property-based tests for state management and notification system
// **Feature: cross-platform-kvm, Property 22: 状态指示器同步更新**
// **Feature: cross-platform-kvm, Property 23: 切换通知触发**

mod common;

use cross_platform_kvm::*;
use cross_platform_kvm::state::*;
use cross_platform_kvm::switch::SwitchEvent;
use proptest::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

// Property test generators

prop_compose! {
    fn arb_device_id()(id in "[a-z0-9]{8,16}") -> String {
        id
    }
}

prop_compose! {
    fn arb_switch_event()(
        device_id in arb_device_id(),
        is_success in any::<bool>(),
        error_msg in "[a-z ]{10,30}"
    ) -> SwitchEvent {
        if is_success {
            SwitchEvent::SwitchedTo(device_id)
        } else {
            SwitchEvent::SwitchFailed(device_id, error_msg)
        }
    }
}

// Helper function to get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// **Property 22: 状态指示器同步更新**
// **Validates: Requirements 7.1, 7.2**
// For any device switch event, all devices' state indicators should update 
// to the current active device within 200 milliseconds
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_22_state_indicator_sync_update(
        device_id in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create notification service and state manager
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = DefaultStateManager::new(notification_service.clone());
            
            // Subscribe to state updates
            let mut state_rx = state_manager.subscribe_state_updates();
            
            // Record the time before state change
            let before_change = current_timestamp_ms();
            
            // Set active device
            state_manager.set_active_device(Some(device_id.clone())).await.unwrap();
            
            // Wait for state update with timeout
            let update = tokio::time::timeout(
                Duration::from_millis(200),
                state_rx.recv()
            ).await;
            
            // Verify update was received within 200ms
            prop_assert!(update.is_ok(), "State update should be received within 200ms");
            
            let update = update.unwrap().unwrap();
            
            // Verify the update contains the correct device
            prop_assert_eq!(update.active_device, Some(device_id.clone()),
                "State update should contain the correct active device");
            
            // Verify the update timestamp is recent
            prop_assert!(update.timestamp >= before_change,
                "Update timestamp should be after the state change");
            
            // Verify the sync delay is within 200ms
            let sync_delay = update.timestamp.saturating_sub(update.state_change_time);
            prop_assert!(sync_delay <= 200,
                "State indicator sync delay should be within 200ms, got {}ms", sync_delay);
            
            // Verify state manager reflects the change
            prop_assert_eq!(state_manager.get_active_device(), Some(device_id),
                "State manager should reflect the active device");
            
            Ok(())
        })?;
    }
}

// **Property 23: 切换通知触发**
// **Validates: Requirements 7.5**
// For any device switch event, if notification functionality is enabled,
// the system should display a switch notification message
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_23_switch_notification_trigger(
        from_device in proptest::option::of(arb_device_id()),
        to_device in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create notification service with notifications enabled
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = DefaultStateManager::new(notification_service.clone());
            
            // Subscribe to notifications
            let mut notification_rx = notification_service.subscribe();
            
            // Create a channel for switch events
            let (switch_tx, switch_rx) = tokio::sync::mpsc::channel(10);
            
            // Start listening to switch events
            state_manager.start_listening(switch_rx).await;
            
            // Set initial device if from_device is Some
            if let Some(ref from_id) = from_device {
                state_manager.set_active_device(Some(from_id.clone())).await.unwrap();
                // Clear any notifications from initial setup
                tokio::time::sleep(Duration::from_millis(10)).await;
                while notification_rx.try_recv().is_ok() {}
            }
            
            // Send a switch event
            let switch_event = SwitchEvent::SwitchedTo(to_device.clone());
            switch_tx.send(switch_event).await.unwrap();
            
            // Wait for notification with timeout
            let notification = tokio::time::timeout(
                Duration::from_millis(500),
                notification_rx.recv()
            ).await;
            
            // Verify notification was received
            prop_assert!(notification.is_ok(), 
                "Notification should be received when notifications are enabled");
            
            let notification = notification.unwrap().unwrap();
            
            // Verify it's a device switched notification
            match notification {
                NotificationEvent::DeviceSwitched { from_device: from, to_device: to, .. } => {
                    prop_assert_eq!(from, from_device,
                        "Notification should contain the correct from_device");
                    prop_assert_eq!(to, to_device,
                        "Notification should contain the correct to_device");
                }
                _ => {
                    return Err(proptest::test_runner::TestCaseError::fail(
                        "Expected DeviceSwitched notification"
                    ));
                }
            }
            
            Ok(())
        })?;
    }
    
    #[test]
    fn property_23_no_notification_when_disabled(
        to_device in arb_device_id()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create notification service with notifications DISABLED
            let notification_service = Arc::new(DefaultNotificationService::new(false));
            let state_manager = DefaultStateManager::new(notification_service.clone());
            
            // Subscribe to notifications
            let mut notification_rx = notification_service.subscribe();
            
            // Create a channel for switch events
            let (switch_tx, switch_rx) = tokio::sync::mpsc::channel(10);
            
            // Start listening to switch events
            state_manager.start_listening(switch_rx).await;
            
            // Send a switch event
            let switch_event = SwitchEvent::SwitchedTo(to_device.clone());
            switch_tx.send(switch_event).await.unwrap();
            
            // Wait a bit to ensure no notification is sent
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // Verify no notification was received
            let notification = notification_rx.try_recv();
            prop_assert!(notification.is_err(),
                "No notification should be received when notifications are disabled");
            
            Ok(())
        })?;
    }
}

// Additional property: State updates should be consistent across multiple subscribers
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_state_update_consistency(
        device_ids in prop::collection::vec(arb_device_id(), 1..5)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = DefaultStateManager::new(notification_service);
            
            // For each device switch, verify state is consistent
            for device_id in device_ids {
                state_manager.set_active_device(Some(device_id.clone())).await.unwrap();
                
                // Verify the state manager reflects the change immediately
                let active = state_manager.get_active_device();
                prop_assert_eq!(active, Some(device_id),
                    "State manager should immediately reflect the active device");
            }
            
            Ok(())
        })?;
    }
}

// Property: Notification timestamps should be monotonically increasing
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_notification_timestamps_monotonic(
        device_ids in prop::collection::vec(arb_device_id(), 2..5)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let notification_service = Arc::new(DefaultNotificationService::new(true));
            let state_manager = DefaultStateManager::new(notification_service.clone());
            
            let mut notification_rx = notification_service.subscribe();
            
            let (switch_tx, switch_rx) = tokio::sync::mpsc::channel(10);
            state_manager.start_listening(switch_rx).await;
            
            let mut last_timestamp = 0u64;
            
            for device_id in device_ids {
                // Send switch event
                switch_tx.send(SwitchEvent::SwitchedTo(device_id)).await.unwrap();
                
                // Receive notification
                let notification = tokio::time::timeout(
                    Duration::from_millis(500),
                    notification_rx.recv()
                ).await.unwrap().unwrap();
                
                // Extract timestamp
                let timestamp = match notification {
                    NotificationEvent::DeviceSwitched { timestamp, .. } => timestamp,
                    _ => panic!("Expected DeviceSwitched notification"),
                };
                
                // Verify timestamp is greater than or equal to last timestamp
                prop_assert!(timestamp >= last_timestamp,
                    "Notification timestamps should be monotonically increasing");
                
                last_timestamp = timestamp;
                
                // Small delay to ensure timestamps differ
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            
            Ok(())
        })?;
    }
}
