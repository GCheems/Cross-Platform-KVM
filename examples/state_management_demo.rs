// Example demonstrating the state management and notification system
// This shows how to integrate the state manager with the switch controller

use cross_platform_kvm::state::*;
use cross_platform_kvm::switch::SwitchEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() {
    println!("=== State Management and Notification System Demo ===\n");
    
    // Create notification service with notifications enabled
    let notification_service = Arc::new(DefaultNotificationService::new(true));
    println!("✓ Created notification service (enabled)");
    
    // Create state manager
    let state_manager = DefaultStateManager::new(notification_service.clone());
    println!("✓ Created state manager\n");
    
    // Subscribe to state updates
    let mut state_rx = state_manager.subscribe_state_updates();
    println!("✓ Subscribed to state updates");
    
    // Subscribe to notifications
    let mut notification_rx = notification_service.subscribe();
    println!("✓ Subscribed to notifications\n");
    
    // Create a channel for switch events
    let (switch_tx, switch_rx) = tokio::sync::mpsc::channel(10);
    
    // Start listening to switch events
    state_manager.start_listening(switch_rx).await;
    println!("✓ State manager listening to switch events\n");
    
    // Spawn a task to monitor state updates
    tokio::spawn(async move {
        while let Some(update) = state_rx.recv().await {
            println!("📊 State Update:");
            println!("   Active Device: {:?}", update.active_device);
            println!("   Timestamp: {}", update.timestamp);
            println!("   Sync Delay: {}ms\n", 
                update.timestamp.saturating_sub(update.state_change_time));
        }
    });
    
    // Spawn a task to monitor notifications
    tokio::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            match notification {
                NotificationEvent::DeviceSwitched { from_device, to_device, timestamp } => {
                    println!("🔔 Notification: Device Switched");
                    println!("   From: {:?}", from_device);
                    println!("   To: {}", to_device);
                    println!("   Time: {}\n", timestamp);
                }
                NotificationEvent::Error { message, timestamp } => {
                    println!("❌ Notification: Error");
                    println!("   Message: {}", message);
                    println!("   Time: {}\n", timestamp);
                }
                _ => {}
            }
        }
    });
    
    // Simulate device switches
    println!("--- Simulating Device Switches ---\n");
    
    // Switch to device 1
    println!("Switching to device-1...");
    switch_tx.send(SwitchEvent::SwitchedTo("device-1".to_string())).await.unwrap();
    time::sleep(Duration::from_millis(300)).await;
    
    // Switch to device 2
    println!("Switching to device-2...");
    switch_tx.send(SwitchEvent::SwitchedTo("device-2".to_string())).await.unwrap();
    time::sleep(Duration::from_millis(300)).await;
    
    // Switch to device 3
    println!("Switching to device-3...");
    switch_tx.send(SwitchEvent::SwitchedTo("device-3".to_string())).await.unwrap();
    time::sleep(Duration::from_millis(300)).await;
    
    // Simulate a failed switch
    println!("Attempting to switch to non-existent device...");
    switch_tx.send(SwitchEvent::SwitchFailed(
        "device-999".to_string(),
        "Device not found".to_string()
    )).await.unwrap();
    time::sleep(Duration::from_millis(300)).await;
    
    // Check current active device
    let active = state_manager.get_active_device();
    println!("Current active device: {:?}", active);
    
    // Check last update time
    if let Some(last_time) = state_manager.get_last_update_time() {
        println!("Last update time: {}", last_time);
    }
    
    println!("\n=== Demo Complete ===");
}
