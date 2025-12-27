// Example demonstrating Windows input injection functionality
// This example shows how to use the InputInjection trait to simulate input events

use cross_platform_kvm::input::{InputInjection, Modifiers, MouseButton};

#[cfg(target_os = "windows")]
use cross_platform_kvm::input::windows::WindowsInputInjection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Input Injection Demo");
    println!("====================\n");

    #[cfg(target_os = "windows")]
    {
        let injector = WindowsInputInjection::new();

        // Example 1: Move mouse to center of screen
        println!("Example 1: Moving mouse to (500, 500)");
        injector.inject_mouse_move(500, 500).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Example 2: Click left mouse button
        println!("Example 2: Clicking left mouse button");
        injector.inject_mouse_button(MouseButton::Left, true).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        injector.inject_mouse_button(MouseButton::Left, false).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Example 3: Scroll mouse wheel
        println!("Example 3: Scrolling mouse wheel down");
        injector.inject_mouse_scroll(0, -3).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Example 4: Type a simple key
        println!("Example 4: Pressing 'A' key");
        injector.inject_key_press(0x41, Modifiers::default(), true).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        injector.inject_key_press(0x41, Modifiers::default(), false).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Example 5: Type with modifiers (Ctrl+C)
        println!("Example 5: Pressing Ctrl+C");
        let modifiers = Modifiers {
            shift: false,
            ctrl: true,
            alt: false,
            meta: false,
        };
        injector.inject_key_press(0x43, modifiers, true).await?; // 'C' key
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        injector.inject_key_press(0x43, modifiers, false).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Example 6: Press Enter key
        println!("Example 6: Pressing Enter key");
        injector.inject_key_press(0x0D, Modifiers::default(), true).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        injector.inject_key_press(0x0D, Modifiers::default(), false).await?;

        println!("\nAll examples completed successfully!");
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("This example only works on Windows.");
        println!("For macOS input injection, see Task 8.");
    }

    Ok(())
}
