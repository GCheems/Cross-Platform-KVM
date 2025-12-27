// macOS-specific input capture implementation using CGEventTap
#![cfg(target_os = "macos")]

use crate::error::{KvmError, Result};
use crate::input::{InputCapture, InputEvent, InputInjection, Modifiers, MouseButton};
use async_trait::async_trait;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::display::CGDisplay;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// macOS implementation of input capture using CGEventTap
pub struct MacOSInputCapture {
    state: Arc<Mutex<CaptureState>>,
}

struct CaptureState {
    event_tap: Option<CFMachPort>,
    event_sender: Option<Sender<InputEvent>>,
    event_receiver: Option<Receiver<InputEvent>>,
    is_capturing: bool,
}

// Wrapper for CFMachPort to make it Send + Sync
struct CFMachPort {
    _inner: core_foundation::mach_port::CFMachPort,
}

unsafe impl Send for CFMachPort {}
unsafe impl Sync for CFMachPort {}

impl MacOSInputCapture {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CaptureState {
                event_tap: None,
                event_sender: None,
                event_receiver: None,
                is_capturing: false,
            })),
        }
    }

    /// Convert CGEventFlags to our Modifiers struct
    fn flags_to_modifiers(flags: CGEventFlags) -> Modifiers {
        Modifiers {
            shift: flags.contains(CGEventFlags::CGEventFlagShift),
            ctrl: flags.contains(CGEventFlags::CGEventFlagControl),
            alt: flags.contains(CGEventFlags::CGEventFlagAlternate),
            meta: flags.contains(CGEventFlags::CGEventFlagCommand),
        }
    }

    /// Convert mouse button number to MouseButton enum
    fn button_number_to_mouse_button(button: i64) -> MouseButton {
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            3 => MouseButton::X1,
            4 => MouseButton::X2,
            _ => MouseButton::Left, // Default to left for unknown buttons
        }
    }
}

#[async_trait]
impl InputCapture for MacOSInputCapture {
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

        // Store the sender in a global for the event tap callback to access
        unsafe {
            GLOBAL_EVENT_SENDER = Some(tx);
        }

        // Create event tap for all input events
        let event_types = vec![
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::MouseMoved,
            CGEventType::LeftMouseDragged,
            CGEventType::RightMouseDragged,
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
            CGEventType::ScrollWheel,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::OtherMouseDragged,
        ];

        let tap = CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            event_types,
            |_proxy, event_type, event| {
                if let Some(sender) = unsafe { &GLOBAL_EVENT_SENDER } {
                    let input_event = match event_type {
                        CGEventType::MouseMoved
                        | CGEventType::LeftMouseDragged
                        | CGEventType::RightMouseDragged
                        | CGEventType::OtherMouseDragged => {
                            let location = event.location();
                            Some(InputEvent::MouseMove {
                                x: location.x as i32,
                                y: location.y as i32,
                            })
                        }
                        CGEventType::LeftMouseDown => Some(InputEvent::MouseButton {
                            button: MouseButton::Left,
                            pressed: true,
                        }),
                        CGEventType::LeftMouseUp => Some(InputEvent::MouseButton {
                            button: MouseButton::Left,
                            pressed: false,
                        }),
                        CGEventType::RightMouseDown => Some(InputEvent::MouseButton {
                            button: MouseButton::Right,
                            pressed: true,
                        }),
                        CGEventType::RightMouseUp => Some(InputEvent::MouseButton {
                            button: MouseButton::Right,
                            pressed: false,
                        }),
                        CGEventType::OtherMouseDown => {
                            let button_number = event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER);
                            let button = MacOSInputCapture::button_number_to_mouse_button(button_number);
                            Some(InputEvent::MouseButton {
                                button,
                                pressed: true,
                            })
                        }
                        CGEventType::OtherMouseUp => {
                            let button_number = event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER);
                            let button = MacOSInputCapture::button_number_to_mouse_button(button_number);
                            Some(InputEvent::MouseButton {
                                button,
                                pressed: false,
                            })
                        }
                        CGEventType::ScrollWheel => {
                            let delta_y = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
                            let delta_x = event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
                            Some(InputEvent::MouseScroll {
                                delta_x: delta_x as i32,
                                delta_y: delta_y as i32,
                            })
                        }
                        CGEventType::KeyDown => {
                            let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                            let flags = event.get_flags();
                            let modifiers = MacOSInputCapture::flags_to_modifiers(flags);
                            Some(InputEvent::KeyPress {
                                key_code,
                                modifiers,
                                pressed: true,
                            })
                        }
                        CGEventType::KeyUp => {
                            let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                            let flags = event.get_flags();
                            let modifiers = MacOSInputCapture::flags_to_modifiers(flags);
                            Some(InputEvent::KeyPress {
                                key_code,
                                modifiers,
                                pressed: false,
                            })
                        }
                        _ => None,
                    };

                    if let Some(evt) = input_event {
                        let _ = sender.try_send(evt);
                    }
                }
                Some(event.to_owned())
            },
        );

        let tap = tap.map_err(|_| {
            KvmError::InputCapture(
                "Failed to create event tap. Please grant accessibility permissions.".to_string(),
            )
        })?;

        // Enable the event tap
        tap.enable();

        // Add the event tap to the run loop
        let run_loop = CFRunLoop::get_current();
        let run_loop_source = tap.mach_port.create_runloop_source(0).map_err(|_| {
            KvmError::InputCapture("Failed to create run loop source".to_string())
        })?;

        run_loop.add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });

        // Store the tap (wrap in our Send+Sync wrapper)
        state.event_tap = Some(CFMachPort { _inner: tap.mach_port });
        state.is_capturing = true;

        // Start the run loop in a separate thread
        std::thread::spawn(move || {
            CFRunLoop::run_current();
        });

        Ok(())
    }

    async fn stop_capture(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();

        if !state.is_capturing {
            return Ok(());
        }

        // Disable the event tap
        if let Some(tap) = state.event_tap.take() {
            // The tap will be automatically disabled when dropped
            drop(tap);
        }

        state.is_capturing = false;
        state.event_sender = None;
        state.event_receiver = None;

        // Clear global sender
        unsafe {
            GLOBAL_EVENT_SENDER = None;
        }

        // Stop the run loop
        CFRunLoop::get_current().stop();

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

// Global sender for event tap callback (CGEventTap requires static functions)
static mut GLOBAL_EVENT_SENDER: Option<Sender<InputEvent>> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_macos_input_capture() {
        let capture = MacOSInputCapture::new();
        let state = capture.state.lock().unwrap();
        assert!(!state.is_capturing);
        assert!(state.event_tap.is_none());
    }

    #[test]
    fn test_flags_to_modifiers() {
        let flags = CGEventFlags::CGEventFlagShift | CGEventFlags::CGEventFlagCommand;
        let modifiers = MacOSInputCapture::flags_to_modifiers(flags);
        assert!(modifiers.shift);
        assert!(modifiers.meta);
        assert!(!modifiers.ctrl);
        assert!(!modifiers.alt);
    }

    #[test]
    fn test_button_number_to_mouse_button() {
        assert_eq!(
            MacOSInputCapture::button_number_to_mouse_button(0),
            MouseButton::Left
        );
        assert_eq!(
            MacOSInputCapture::button_number_to_mouse_button(1),
            MouseButton::Right
        );
        assert_eq!(
            MacOSInputCapture::button_number_to_mouse_button(2),
            MouseButton::Middle
        );
        assert_eq!(
            MacOSInputCapture::button_number_to_mouse_button(3),
            MouseButton::X1
        );
        assert_eq!(
            MacOSInputCapture::button_number_to_mouse_button(4),
            MouseButton::X2
        );
    }
}

/// macOS implementation of input injection using CGEventPost API
pub struct MacOSInputInjection {
    // We don't store the event source since it's not Send/Sync
    // Instead we create it on demand
}

// Manually implement Send + Sync since we don't store any non-Send/Sync fields
unsafe impl Send for MacOSInputInjection {}
unsafe impl Sync for MacOSInputInjection {}

impl MacOSInputInjection {
    pub fn new() -> Result<Self> {
        // Just create the struct - we'll create event sources on demand
        Ok(Self {})
    }
    
    /// Create an event source for generating events
    fn create_event_source() -> Result<CGEventSource> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| KvmError::InputInjection(
                "Failed to create CGEventSource".to_string()
            ))
    }

    /// Convert our Modifiers struct to CGEventFlags
    fn modifiers_to_flags(modifiers: &Modifiers) -> CGEventFlags {
        let mut flags = CGEventFlags::empty();
        
        if modifiers.shift {
            flags |= CGEventFlags::CGEventFlagShift;
        }
        if modifiers.ctrl {
            flags |= CGEventFlags::CGEventFlagControl;
        }
        if modifiers.alt {
            flags |= CGEventFlags::CGEventFlagAlternate;
        }
        if modifiers.meta {
            flags |= CGEventFlags::CGEventFlagCommand;
        }
        
        flags
    }

    /// Map a generic key code to macOS key code
    /// This handles cross-platform key code translation
    fn map_key_code_to_macos(key_code: u32) -> u16 {
        // For macOS, we need to translate key codes
        // Common macOS key codes (from Carbon.framework HIToolbox/Events.h)
        match key_code {
            // Windows VK codes to macOS key codes translation
            // Letters A-Z (Windows: 0x41-0x5A, macOS: 0x00-0x0C for QWERTY layout)
            0x41 => 0x00, // A
            0x42 => 0x0B, // B
            0x43 => 0x08, // C
            0x44 => 0x02, // D
            0x45 => 0x0E, // E
            0x46 => 0x03, // F
            0x47 => 0x05, // G
            0x48 => 0x04, // H
            0x49 => 0x22, // I
            0x4A => 0x26, // J
            0x4B => 0x28, // K
            0x4C => 0x25, // L
            0x4D => 0x2E, // M
            0x4E => 0x2D, // N
            0x4F => 0x1F, // O
            0x50 => 0x23, // P
            0x51 => 0x0C, // Q
            0x52 => 0x0F, // R
            0x53 => 0x01, // S
            0x54 => 0x11, // T
            0x55 => 0x20, // U
            0x56 => 0x09, // V
            0x57 => 0x0D, // W
            0x58 => 0x07, // X
            0x59 => 0x10, // Y
            0x5A => 0x06, // Z
            
            // Numbers 0-9 (Windows: 0x30-0x39, macOS: 0x1D, 0x12-0x13, 0x14-0x19)
            0x30 => 0x1D, // 0
            0x31 => 0x12, // 1
            0x32 => 0x13, // 2
            0x33 => 0x14, // 3
            0x34 => 0x15, // 4
            0x35 => 0x17, // 5
            0x36 => 0x16, // 6
            0x37 => 0x1A, // 7
            0x38 => 0x1C, // 8
            0x39 => 0x19, // 9
            
            // Function keys F1-F12
            0x70 => 0x7A, // F1
            0x71 => 0x78, // F2
            0x72 => 0x63, // F3
            0x73 => 0x76, // F4
            0x74 => 0x60, // F5
            0x75 => 0x61, // F6
            0x76 => 0x62, // F7
            0x77 => 0x64, // F8
            0x78 => 0x65, // F9
            0x79 => 0x6D, // F10
            0x7A => 0x67, // F11
            0x7B => 0x6F, // F12
            
            // Special keys
            0x08 => 0x33, // Backspace
            0x09 => 0x30, // Tab
            0x0D => 0x24, // Return/Enter
            0x1B => 0x35, // Escape
            0x20 => 0x31, // Space
            0x21 => 0x74, // Page Up
            0x22 => 0x79, // Page Down
            0x23 => 0x77, // End
            0x24 => 0x73, // Home
            0x25 => 0x7B, // Left Arrow
            0x26 => 0x7E, // Up Arrow
            0x27 => 0x7C, // Right Arrow
            0x28 => 0x7D, // Down Arrow
            0x2D => 0x72, // Insert (Help on Mac)
            0x2E => 0x75, // Delete (Forward Delete)
            
            // Modifier keys
            0x10 => 0x38, // Shift (Left)
            0x11 => 0x3B, // Control (Left)
            0x12 => 0x3A, // Alt/Option (Left)
            0x5B => 0x37, // Command (Left)
            0x5C => 0x37, // Command (Right - map to left)
            
            // Punctuation and symbols
            0xBA => 0x29, // ; :
            0xBB => 0x18, // = +
            0xBC => 0x2B, // , <
            0xBD => 0x1B, // - _
            0xBE => 0x2F, // . >
            0xBF => 0x2C, // / ?
            0xC0 => 0x32, // ` ~
            0xDB => 0x21, // [ {
            0xDC => 0x2A, // \ |
            0xDD => 0x1E, // ] }
            0xDE => 0x27, // ' "
            
            // If the key_code is already a macOS key code (< 256), use it directly
            0..=255 => key_code as u16,
            
            // Default: use as-is if it's a reasonable value
            _ => (key_code & 0xFF) as u16,
        }
    }
}

#[async_trait]
impl InputInjection for MacOSInputInjection {
    async fn inject_mouse_move(&self, x: i32, y: i32) -> Result<()> {
        // Create event source
        let event_source = Self::create_event_source()?;
        
        // Create a mouse move event
        let event = CGEvent::new_mouse_event(
            event_source,
            CGEventType::MouseMoved,
            core_graphics::geometry::CGPoint::new(x as f64, y as f64),
            core_graphics::event::CGMouseButton::Left, // Button doesn't matter for move
        ).map_err(|_| KvmError::InputInjection(
            "Failed to create mouse move event".to_string()
        ))?;

        // Post the event to the system
        event.post(CGEventTapLocation::HID);

        Ok(())
    }

    async fn inject_mouse_button(&self, button: MouseButton, pressed: bool) -> Result<()> {
        // Create event source
        let event_source = Self::create_event_source()?;
        
        // Get current mouse location
        let location = CGDisplay::main().bounds();
        let current_location = core_graphics::geometry::CGPoint::new(
            location.size.width / 2.0,
            location.size.height / 2.0,
        );

        // Map our button type to CGMouseButton and event types
        let (cg_button, event_type) = match (button, pressed) {
            (MouseButton::Left, true) => (
                core_graphics::event::CGMouseButton::Left,
                CGEventType::LeftMouseDown,
            ),
            (MouseButton::Left, false) => (
                core_graphics::event::CGMouseButton::Left,
                CGEventType::LeftMouseUp,
            ),
            (MouseButton::Right, true) => (
                core_graphics::event::CGMouseButton::Right,
                CGEventType::RightMouseDown,
            ),
            (MouseButton::Right, false) => (
                core_graphics::event::CGMouseButton::Right,
                CGEventType::RightMouseUp,
            ),
            (MouseButton::Middle, true) => (
                core_graphics::event::CGMouseButton::Center,
                CGEventType::OtherMouseDown,
            ),
            (MouseButton::Middle, false) => (
                core_graphics::event::CGMouseButton::Center,
                CGEventType::OtherMouseUp,
            ),
            (MouseButton::X1, true) => (
                core_graphics::event::CGMouseButton::Center, // Use center for extra buttons
                CGEventType::OtherMouseDown,
            ),
            (MouseButton::X1, false) => (
                core_graphics::event::CGMouseButton::Center,
                CGEventType::OtherMouseUp,
            ),
            (MouseButton::X2, true) => (
                core_graphics::event::CGMouseButton::Center,
                CGEventType::OtherMouseDown,
            ),
            (MouseButton::X2, false) => (
                core_graphics::event::CGMouseButton::Center,
                CGEventType::OtherMouseUp,
            ),
        };

        // Create the mouse button event
        let event = CGEvent::new_mouse_event(
            event_source,
            event_type,
            current_location,
            cg_button,
        ).map_err(|_| KvmError::InputInjection(
            format!("Failed to create mouse button event for {:?}", button)
        ))?;

        // For X1 and X2 buttons, set the button number
        if matches!(button, MouseButton::X1 | MouseButton::X2) {
            let button_number = match button {
                MouseButton::X1 => 3,
                MouseButton::X2 => 4,
                _ => 2,
            };
            event.set_integer_value_field(
                EventField::MOUSE_EVENT_BUTTON_NUMBER,
                button_number,
            );
        }

        // Post the event
        event.post(CGEventTapLocation::HID);

        Ok(())
    }

    async fn inject_mouse_scroll(&self, delta_x: i32, delta_y: i32) -> Result<()> {
        // Create event source
        let event_source = Self::create_event_source()?;
        
        // Create a scroll wheel event using the generic CGEvent::new
        // Then set the scroll wheel fields manually
        let event = CGEvent::new(event_source)
            .map_err(|_| KvmError::InputInjection(
                "Failed to create scroll event".to_string()
            ))?;
        
        // Set the event type to scroll wheel
        event.set_type(CGEventType::ScrollWheel);
        
        // Set scroll deltas
        // Axis 1 is vertical (Y), Axis 2 is horizontal (X)
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1, delta_y as i64);
        event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2, delta_x as i64);

        // Post the event
        event.post(CGEventTapLocation::HID);

        Ok(())
    }

    async fn inject_key_press(
        &self,
        key_code: u32,
        modifiers: Modifiers,
        pressed: bool,
    ) -> Result<()> {
        // Create event source
        let event_source = Self::create_event_source()?;
        
        // Map the key code to macOS key code
        let macos_key_code = Self::map_key_code_to_macos(key_code);

        // Create the keyboard event
        let event = CGEvent::new_keyboard_event(
            event_source,
            macos_key_code,
            pressed,
        ).map_err(|_| KvmError::InputInjection(
            format!("Failed to create keyboard event for key code {}", key_code)
        ))?;

        // Set the modifier flags
        let flags = Self::modifiers_to_flags(&modifiers);
        event.set_flags(flags);

        // Post the event
        event.post(CGEventTapLocation::HID);

        Ok(())
    }
}

#[cfg(test)]
mod injection_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_macos_input_injection() {
        // This test may fail if not running on macOS or without proper permissions
        // We'll make it conditional
        #[cfg(target_os = "macos")]
        {
            let result = MacOSInputInjection::new();
            // On macOS, this should succeed
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_modifiers_to_flags() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        
        let flags = MacOSInputInjection::modifiers_to_flags(&modifiers);
        assert!(flags.contains(CGEventFlags::CGEventFlagShift));
        assert!(flags.contains(CGEventFlags::CGEventFlagAlternate));
        assert!(!flags.contains(CGEventFlags::CGEventFlagControl));
        assert!(!flags.contains(CGEventFlags::CGEventFlagCommand));
    }

    #[test]
    fn test_map_key_code_to_macos() {
        // Test letter mapping
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x41), 0x00); // A
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x5A), 0x06); // Z
        
        // Test number mapping
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x30), 0x1D); // 0
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x31), 0x12); // 1
        
        // Test special key mapping
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x0D), 0x24); // Enter
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x20), 0x31); // Space
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x1B), 0x35); // Escape
        
        // Test arrow keys
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x25), 0x7B); // Left
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x26), 0x7E); // Up
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x27), 0x7C); // Right
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x28), 0x7D); // Down
    }

    #[test]
    fn test_all_modifier_combinations() {
        let test_cases = vec![
            (Modifiers { shift: false, ctrl: false, alt: false, meta: false }, CGEventFlags::empty()),
            (Modifiers { shift: true, ctrl: false, alt: false, meta: false }, CGEventFlags::CGEventFlagShift),
            (Modifiers { shift: false, ctrl: true, alt: false, meta: false }, CGEventFlags::CGEventFlagControl),
            (Modifiers { shift: false, ctrl: false, alt: true, meta: false }, CGEventFlags::CGEventFlagAlternate),
            (Modifiers { shift: false, ctrl: false, alt: false, meta: true }, CGEventFlags::CGEventFlagCommand),
        ];

        for (modifiers, expected_flag) in test_cases {
            let flags = MacOSInputInjection::modifiers_to_flags(&modifiers);
            if expected_flag.is_empty() {
                assert!(flags.is_empty() || flags.bits() == 0);
            } else {
                assert!(flags.contains(expected_flag));
            }
        }
    }

    #[test]
    fn test_function_key_mapping() {
        // Test F1-F12 mapping
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x70), 0x7A); // F1
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x71), 0x78); // F2
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0x7B), 0x6F); // F12
    }

    #[test]
    fn test_punctuation_key_mapping() {
        // Test punctuation keys
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0xBA), 0x29); // ;
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0xBB), 0x18); // =
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0xBC), 0x2B); // ,
        assert_eq!(MacOSInputInjection::map_key_code_to_macos(0xBD), 0x1B); // -
    }
}
