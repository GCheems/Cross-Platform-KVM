// Windows-specific input capture and injection implementation
#![cfg(target_os = "windows")]

use crate::error::{KvmError, Result};
use crate::input::{InputCapture, InputEvent, InputInjection, Modifiers, MouseButton};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows implementation of input capture using low-level hooks
pub struct WindowsInputCapture {
    state: Arc<Mutex<CaptureState>>,
}

struct CaptureState {
    mouse_hook: Option<HHOOK>,
    keyboard_hook: Option<HHOOK>,
    event_sender: Option<Sender<InputEvent>>,
    event_receiver: Option<Receiver<InputEvent>>,
    is_capturing: bool,
}

impl WindowsInputCapture {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CaptureState {
                mouse_hook: None,
                keyboard_hook: None,
                event_sender: None,
                event_receiver: None,
                is_capturing: false,
            })),
        }
    }

    /// Get the current modifier key states
    fn get_modifiers() -> Modifiers {
        unsafe {
            Modifiers {
                shift: GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0,
                ctrl: GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0,
                alt: GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0,
                meta: GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000 != 0
                    || GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000 != 0,
            }
        }
    }
}

#[async_trait]
impl InputCapture for WindowsInputCapture {
    async fn start_capture(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        if state.is_capturing {
            return Err(KvmError::InvalidState(
                "Input capture already started".to_string(),
            ));
        }

        // Create channel for events
        let (tx, rx) = channel(1000);
        state.event_sender = Some(tx.clone());
        state.event_receiver = Some(rx);

        // Install mouse hook
        unsafe {
            let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0)
                .map_err(|e| {
                    KvmError::InputCapture(format!("Failed to install mouse hook: {:?}", e))
                })?;

            state.mouse_hook = Some(mouse_hook);

            // Install keyboard hook
            let keyboard_hook =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0).map_err(
                    |e| {
                        // Clean up mouse hook if keyboard hook fails
                        if let Some(hook) = state.mouse_hook {
                            let _ = UnhookWindowsHookEx(hook);
                        }
                        KvmError::InputCapture(format!("Failed to install keyboard hook: {:?}", e))
                    },
                )?;

            state.keyboard_hook = Some(keyboard_hook);
        }

        state.is_capturing = true;

        // Store the sender in a global for the hook procedures to access
        unsafe {
            GLOBAL_EVENT_SENDER = Some(tx);
        }

        Ok(())
    }

    async fn stop_capture(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        if !state.is_capturing {
            return Ok(());
        }

        // Unhook mouse
        if let Some(hook) = state.mouse_hook.take() {
            unsafe {
                UnhookWindowsHookEx(hook).map_err(|e| {
                    KvmError::InputCapture(format!("Failed to unhook mouse: {:?}", e))
                })?;
            }
        }

        // Unhook keyboard
        if let Some(hook) = state.keyboard_hook.take() {
            unsafe {
                UnhookWindowsHookEx(hook).map_err(|e| {
                    KvmError::InputCapture(format!("Failed to unhook keyboard: {:?}", e))
                })?;
            }
        }

        state.is_capturing = false;
        state.event_sender = None;
        state.event_receiver = None;

        // Clear global sender
        unsafe {
            GLOBAL_EVENT_SENDER = None;
        }

        Ok(())
    }

    fn subscribe(&self) -> Receiver<InputEvent> {
        let mut state = self.state.lock().unwrap();
        
        // If we already have a receiver, take it out
        if let Some(rx) = state.event_receiver.take() {
            return rx;
        }
        
        // Otherwise create a new channel
        let (tx, rx) = channel(1000);
        state.event_sender = Some(tx.clone());

        // Update global sender if capturing
        if state.is_capturing {
            unsafe {
                GLOBAL_EVENT_SENDER = Some(tx);
            }
        }

        rx
    }
}

// Global sender for hook procedures (Windows hooks require static functions)
static mut GLOBAL_EVENT_SENDER: Option<Sender<InputEvent>> = None;

/// Low-level mouse hook procedure
unsafe extern "system" fn mouse_hook_proc(
    ncode: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if ncode >= 0 {
        if let Some(sender) = &GLOBAL_EVENT_SENDER {
            let mouse_data = &*(lparam.0 as *const MSLLHOOKSTRUCT);

            let event = match wparam.0 as u32 {
                WM_MOUSEMOVE => Some(InputEvent::MouseMove {
                    x: mouse_data.pt.x,
                    y: mouse_data.pt.y,
                }),
                WM_LBUTTONDOWN => Some(InputEvent::MouseButton {
                    button: MouseButton::Left,
                    pressed: true,
                }),
                WM_LBUTTONUP => Some(InputEvent::MouseButton {
                    button: MouseButton::Left,
                    pressed: false,
                }),
                WM_RBUTTONDOWN => Some(InputEvent::MouseButton {
                    button: MouseButton::Right,
                    pressed: true,
                }),
                WM_RBUTTONUP => Some(InputEvent::MouseButton {
                    button: MouseButton::Right,
                    pressed: false,
                }),
                WM_MBUTTONDOWN => Some(InputEvent::MouseButton {
                    button: MouseButton::Middle,
                    pressed: true,
                }),
                WM_MBUTTONUP => Some(InputEvent::MouseButton {
                    button: MouseButton::Middle,
                    pressed: false,
                }),
                WM_XBUTTONDOWN => {
                    let hi_word = (mouse_data.mouseData >> 16) & 0xFFFF;
                    let button = if hi_word == 1 {
                        MouseButton::X1
                    } else {
                        MouseButton::X2
                    };
                    Some(InputEvent::MouseButton {
                        button,
                        pressed: true,
                    })
                }
                WM_XBUTTONUP => {
                    let hi_word = (mouse_data.mouseData >> 16) & 0xFFFF;
                    let button = if hi_word == 1 {
                        MouseButton::X1
                    } else {
                        MouseButton::X2
                    };
                    Some(InputEvent::MouseButton {
                        button,
                        pressed: false,
                    })
                }
                WM_MOUSEWHEEL => {
                    let delta = ((mouse_data.mouseData >> 16) & 0xFFFF) as i16 as i32;
                    Some(InputEvent::MouseScroll {
                        delta_x: 0,
                        delta_y: delta / 120, // WHEEL_DELTA is 120
                    })
                }
                WM_MOUSEHWHEEL => {
                    let delta = ((mouse_data.mouseData >> 16) & 0xFFFF) as i16 as i32;
                    Some(InputEvent::MouseScroll {
                        delta_x: delta / 120,
                        delta_y: 0,
                    })
                }
                _ => None,
            };

            if let Some(event) = event {
                // Send event asynchronously (non-blocking)
                let _ = sender.try_send(event);
            }
        }
    }

    CallNextHookEx(None, ncode, wparam, lparam)
}

/// Low-level keyboard hook procedure
unsafe extern "system" fn keyboard_hook_proc(
    ncode: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if ncode >= 0 {
        if let Some(sender) = &GLOBAL_EVENT_SENDER {
            let keyboard_data = &*(lparam.0 as *const KBDLLHOOKSTRUCT);

            let pressed = match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => true,
                WM_KEYUP | WM_SYSKEYUP => false,
                _ => return CallNextHookEx(None, ncode, wparam, lparam),
            };

            let modifiers = WindowsInputCapture::get_modifiers();

            let event = InputEvent::KeyPress {
                key_code: keyboard_data.vkCode,
                modifiers,
                pressed,
            };

            // Send event asynchronously (non-blocking)
            let _ = sender.try_send(event);
        }
    }

    CallNextHookEx(None, ncode, wparam, lparam)
}

/// Windows implementation of input injection using SendInput API
pub struct WindowsInputInjection;

impl WindowsInputInjection {
    pub fn new() -> Self {
        Self
    }

    /// Get the current screen dimensions
    fn get_screen_dimensions() -> (i32, i32) {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (width, height)
        }
    }

    /// Convert absolute coordinates to normalized coordinates (0-65535)
    fn normalize_coordinates(x: i32, y: i32) -> (i32, i32) {
        let (screen_width, screen_height) = Self::get_screen_dimensions();
        let normalized_x = (x * 65535) / screen_width;
        let normalized_y = (y * 65535) / screen_height;
        (normalized_x, normalized_y)
    }
}

#[async_trait]
impl InputInjection for WindowsInputInjection {
    async fn inject_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        let (normalized_x, normalized_y) = Self::normalize_coordinates(x, y);

        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: normalized_x,
                    dy: normalized_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            let result = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if result == 0 {
                return Err(KvmError::InputInjection(
                    "Failed to inject mouse move event".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn inject_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        let (down_flag, up_flag, mouse_data) = match button {
            MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
            MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
            MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
            MouseButton::X1 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON1 as u32),
            MouseButton::X2 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON2 as u32),
        };

        let flags = if pressed { down_flag } else { up_flag };

        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            let result = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            if result == 0 {
                return Err(KvmError::InputInjection(
                    "Failed to inject mouse button event".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        // Inject vertical scroll if delta_y is non-zero
        if delta_y != 0 {
            let mut input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: (delta_y * 120) as u32, // WHEEL_DELTA is 120
                        dwFlags: MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            unsafe {
                let result = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
                if result == 0 {
                    return Err(KvmError::InputInjection(
                        "Failed to inject vertical scroll event".to_string(),
                    ));
                }
            }
        }

        // Inject horizontal scroll if delta_x is non-zero
        if delta_x != 0 {
            let mut input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: (delta_x * 120) as u32, // WHEEL_DELTA is 120
                        dwFlags: MOUSEEVENTF_HWHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };

            unsafe {
                let result = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
                if result == 0 {
                    return Err(KvmError::InputInjection(
                        "Failed to inject horizontal scroll event".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    async fn inject_key_press(
        &self,
        key_code: u32,
        modifiers: Modifiers,
        pressed: bool,
    ) -> Result<()> {
        // Map the key code to Windows virtual key code
        let vk_code = map_key_code_to_vk(key_code);

        // Build a list of inputs to send
        let mut inputs = Vec::new();

        // If pressing, inject modifier keys first
        if pressed {
            if modifiers.shift && !is_modifier_pressed(VK_SHIFT) {
                inputs.push(create_key_input(VK_SHIFT.0 as u16, true));
            }
            if modifiers.ctrl && !is_modifier_pressed(VK_CONTROL) {
                inputs.push(create_key_input(VK_CONTROL.0 as u16, true));
            }
            if modifiers.alt && !is_modifier_pressed(VK_MENU) {
                inputs.push(create_key_input(VK_MENU.0 as u16, true));
            }
            if modifiers.meta && !is_modifier_pressed(VK_LWIN) {
                inputs.push(create_key_input(VK_LWIN.0 as u16, true));
            }
        }

        // Inject the main key
        inputs.push(create_key_input(vk_code, pressed));

        // If releasing, release modifier keys after
        if !pressed {
            if modifiers.meta && is_modifier_pressed(VK_LWIN) {
                inputs.push(create_key_input(VK_LWIN.0 as u16, false));
            }
            if modifiers.alt && is_modifier_pressed(VK_MENU) {
                inputs.push(create_key_input(VK_MENU.0 as u16, false));
            }
            if modifiers.ctrl && is_modifier_pressed(VK_CONTROL) {
                inputs.push(create_key_input(VK_CONTROL.0 as u16, false));
            }
            if modifiers.shift && is_modifier_pressed(VK_SHIFT) {
                inputs.push(create_key_input(VK_SHIFT.0 as u16, false));
            }
        }

        unsafe {
            let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if result == 0 {
                return Err(KvmError::InputInjection(
                    "Failed to inject key press event".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Create a keyboard INPUT structure
fn create_key_input(vk_code: u16, pressed: bool) -> INPUT {
    let flags = if pressed {
        KEYBD_EVENT_FLAGS(0)
    } else {
        KEYEVENTF_KEYUP
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk_code),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Check if a modifier key is currently pressed
fn is_modifier_pressed(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

/// Map a generic key code to Windows virtual key code
/// This function handles the key code mapping between platforms
fn map_key_code_to_vk(key_code: u32) -> u16 {
    // For Windows, the key_code is already a Windows virtual key code
    // This function exists to allow for future cross-platform key code translation
    // If we receive key codes from macOS, we would translate them here
    
    // Common key mappings (Windows VK codes)
    match key_code {
        // Letters A-Z (0x41-0x5A)
        0x41..=0x5A => key_code as u16,
        
        // Numbers 0-9 (0x30-0x39)
        0x30..=0x39 => key_code as u16,
        
        // Function keys F1-F24 (0x70-0x87)
        0x70..=0x87 => key_code as u16,
        
        // Numpad keys (0x60-0x6F)
        0x60..=0x6F => key_code as u16,
        
        // Special keys
        0x08 => VK_BACK.0 as u16,      // Backspace
        0x09 => VK_TAB.0 as u16,       // Tab
        0x0D => VK_RETURN.0 as u16,    // Enter
        0x1B => VK_ESCAPE.0 as u16,    // Escape
        0x20 => VK_SPACE.0 as u16,     // Space
        0x21 => VK_PRIOR.0 as u16,     // Page Up
        0x22 => VK_NEXT.0 as u16,      // Page Down
        0x23 => VK_END.0 as u16,       // End
        0x24 => VK_HOME.0 as u16,      // Home
        0x25 => VK_LEFT.0 as u16,      // Left Arrow
        0x26 => VK_UP.0 as u16,        // Up Arrow
        0x27 => VK_RIGHT.0 as u16,     // Right Arrow
        0x28 => VK_DOWN.0 as u16,      // Down Arrow
        0x2C => VK_SNAPSHOT.0 as u16,  // Print Screen
        0x2D => VK_INSERT.0 as u16,    // Insert
        0x2E => VK_DELETE.0 as u16,    // Delete
        
        // Modifier keys
        0x10 => VK_SHIFT.0 as u16,     // Shift
        0x11 => VK_CONTROL.0 as u16,   // Control
        0x12 => VK_MENU.0 as u16,      // Alt
        0x5B => VK_LWIN.0 as u16,      // Left Windows
        0x5C => VK_RWIN.0 as u16,      // Right Windows
        
        // OEM keys (punctuation, etc.)
        0xBA => VK_OEM_1.0 as u16,     // ;:
        0xBB => VK_OEM_PLUS.0 as u16,  // =+
        0xBC => VK_OEM_COMMA.0 as u16, // ,<
        0xBD => VK_OEM_MINUS.0 as u16, // -_
        0xBE => VK_OEM_PERIOD.0 as u16,// .>
        0xBF => VK_OEM_2.0 as u16,     // /?
        0xC0 => VK_OEM_3.0 as u16,     // `~
        0xDB => VK_OEM_4.0 as u16,     // [{
        0xDC => VK_OEM_5.0 as u16,     // \|
        0xDD => VK_OEM_6.0 as u16,     // ]}
        0xDE => VK_OEM_7.0 as u16,     // '"
        
        // Default: pass through as-is
        _ => key_code as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_windows_input_capture() {
        let capture = WindowsInputCapture::new();
        let state = capture.state.lock().unwrap();
        assert!(!state.is_capturing);
        assert!(state.mouse_hook.is_none());
        assert!(state.keyboard_hook.is_none());
    }

    #[tokio::test]
    async fn test_get_modifiers() {
        // This test just ensures the function doesn't crash
        let _modifiers = WindowsInputCapture::get_modifiers();
    }

    #[tokio::test]
    async fn test_create_windows_input_injection() {
        let _injection = WindowsInputInjection::new();
    }

    #[tokio::test]
    async fn test_normalize_coordinates() {
        let (norm_x, norm_y) = WindowsInputInjection::normalize_coordinates(100, 100);
        // Normalized coordinates should be in range 0-65535
        assert!(norm_x >= 0 && norm_x <= 65535);
        assert!(norm_y >= 0 && norm_y <= 65535);
    }

    #[tokio::test]
    async fn test_map_key_code_to_vk() {
        // Test letter mapping
        assert_eq!(map_key_code_to_vk(0x41), 0x41); // 'A'
        
        // Test number mapping
        assert_eq!(map_key_code_to_vk(0x30), 0x30); // '0'
        
        // Test special key mapping
        assert_eq!(map_key_code_to_vk(0x0D), VK_RETURN.0 as u16); // Enter
        assert_eq!(map_key_code_to_vk(0x20), VK_SPACE.0 as u16);  // Space
    }

    #[test]
    fn test_create_key_input() {
        let input = create_key_input(0x41, true); // 'A' key pressed
        assert_eq!(input.r#type, INPUT_KEYBOARD);
    }
}
