// Example demonstrating Windows input capture
// This example shows how to use the input capture API

use cross_platform_kvm::input::{create_input_capture, InputCapture, InputEvent};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> cross_platform_kvm::Result<()> {
    env_logger::init();

    println!("Creating input capture...");
    let capture = create_input_capture();

    println!("Subscribing to input events...");
    let mut rx = capture.subscribe();

    println!("Starting input capture...");
    capture.start_capture().await?;

    println!("Capturing input for 10 seconds. Move your mouse and press keys...");

    // Spawn a task to receive events
    let event_task = tokio::spawn(async move {
        let mut event_count = 0;
        while let Some(event) = rx.recv().await {
            event_count += 1;
            match event {
                InputEvent::MouseMove { x, y } => {
                    if event_count % 100 == 0 {
                        // Only print every 100th mouse move to avoid spam
                        println!("Mouse moved to ({}, {})", x, y);
                    }
                }
                InputEvent::MouseButton { button, pressed } => {
                    println!("Mouse button {:?} {}", button, if pressed { "pressed" } else { "released" });
                }
                InputEvent::MouseScroll { delta_x, delta_y } => {
                    println!("Mouse scroll: dx={}, dy={}", delta_x, delta_y);
                }
                InputEvent::KeyPress { key_code, modifiers, pressed } => {
                    println!(
                        "Key {} (code: {}) - Shift: {}, Ctrl: {}, Alt: {}, Meta: {}",
                        if pressed { "pressed" } else { "released" },
                        key_code,
                        modifiers.shift,
                        modifiers.ctrl,
                        modifiers.alt,
                        modifiers.meta
                    );
                }
            }
        }
        println!("Total events captured: {}", event_count);
    });

    // Wait for 10 seconds
    sleep(Duration::from_secs(10)).await;

    println!("Stopping input capture...");
    capture.stop_capture().await?;

    // Wait for event task to finish
    event_task.await.unwrap();

    println!("Done!");
    Ok(())
}
