use crate::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

// Platform-specific implementations
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// Keyboard modifier keys
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Command on macOS, Windows key on Windows
}

/// Input events that can be captured or injected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseScroll { delta_x: i32, delta_y: i32 },
    KeyPress { key_code: u32, modifiers: Modifiers, pressed: bool },
}

/// Trait for capturing input events from the local system
#[async_trait::async_trait]
pub trait InputCapture: Send + Sync {
    /// Start capturing input events
    async fn start_capture(&self) -> Result<()>;

    /// Stop capturing input events
    async fn stop_capture(&self) -> Result<()>;

    /// Subscribe to captured input events
    fn subscribe(&self) -> Receiver<InputEvent>;
}

/// Trait for injecting input events into the local system
#[async_trait::async_trait]
pub trait InputInjection: Send + Sync {
    /// Inject a mouse move event
    async fn inject_mouse_move(&self, x: i32, y: i32) -> Result<()>;

    /// Inject a mouse button event
    async fn inject_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()>;

    /// Inject a mouse scroll event
    async fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()>;

    /// Inject a keyboard event
    async fn inject_key_press(&self, key_code: u32, modifiers: Modifiers, pressed: bool) -> Result<()>;
}

/// Create a platform-specific input capture implementation
#[cfg(target_os = "windows")]
pub fn create_input_capture() -> Box<dyn InputCapture> {
    Box::new(windows::WindowsInputCapture::new())
}

/// Create a platform-specific input capture implementation
#[cfg(target_os = "macos")]
pub fn create_input_capture() -> Box<dyn InputCapture> {
    Box::new(macos::MacOSInputCapture::new())
}

/// Create a platform-specific input capture implementation
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn create_input_capture() -> Box<dyn InputCapture> {
    panic!("Unsupported platform for input capture")
}

/// Create a platform-specific input injection implementation
#[cfg(target_os = "windows")]
pub fn create_input_injection() -> Box<dyn InputInjection> {
    Box::new(windows::WindowsInputInjection::new())
}

/// Create a platform-specific input injection implementation
#[cfg(target_os = "macos")]
pub fn create_input_injection() -> Box<dyn InputInjection> {
    Box::new(macos::MacOSInputInjection::new().expect("Failed to create macOS input injection"))
}

/// Create a platform-specific input injection implementation
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn create_input_injection() -> Box<dyn InputInjection> {
    panic!("Unsupported platform for input injection")
}
