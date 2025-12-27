// Unit tests for data models and binary protocol serialization
// Tests various input events encoding/decoding and edge cases

mod common;

use cross_platform_kvm::*;
use cross_platform_kvm::protocol::*;
use std::io::Cursor;

#[cfg(test)]
mod protocol_tests {
    use super::*;

    // Test basic message header serialization
    #[test]
    fn test_message_header_serialization() {
        let header = MessageHeader::new(EventType::MouseMove, 100);
        let bytes = header.to_bytes();
        
        assert_eq!(bytes.len(), 8);
        assert_eq!(bytes[0..2], MAGIC_NUMBER.to_be_bytes());
        assert_eq!(bytes[2], PROTOCOL_VERSION);
        assert_eq!(bytes[3], EventType::MouseMove as u8);
    }

    #[test]
    fn test_message_header_deserialization() {
        let header = MessageHeader::new(EventType::KeyPress, 256);
        let bytes = header.to_bytes();
        let decoded = MessageHeader::from_bytes(&bytes).unwrap();
        
        assert_eq!(decoded.magic, MAGIC_NUMBER);
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.event_type, EventType::KeyPress);
        assert_eq!(decoded.payload_length, 256);
    }

    #[test]
    fn test_invalid_magic_number_rejected() {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&0x1234u16.to_be_bytes());
        bytes[2] = PROTOCOL_VERSION;
        bytes[3] = EventType::MouseMove as u8;
        
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_version_rejected() {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&MAGIC_NUMBER.to_be_bytes());
        bytes[2] = 0xFF; // Invalid version
        bytes[3] = EventType::MouseMove as u8;
        
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_event_type_rejected() {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&MAGIC_NUMBER.to_be_bytes());
        bytes[2] = PROTOCOL_VERSION;
        bytes[3] = 0xFF; // Invalid event type
        
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_too_short() {
        let bytes = [0u8; 4]; // Only 4 bytes instead of 8
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    // Test MouseMove event encoding/decoding
    #[test]
    fn test_mouse_move_encoding() {
        let event = input::InputEvent::MouseMove { x: 100, y: 200 };
        let message = encode_input_event(&event).unwrap();
        
        assert_eq!(message.header.event_type, EventType::MouseMove);
        assert_eq!(message.payload.len(), 8);
    }

    #[test]
    fn test_mouse_move_roundtrip() {
        let event = input::InputEvent::MouseMove { x: 1920, y: 1080 };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseMove { x, y } => {
                assert_eq!(x, 1920);
                assert_eq!(y, 1080);
            }
            _ => panic!("Expected MouseMove event"),
        }
    }

    #[test]
    fn test_mouse_move_negative_coordinates() {
        let event = input::InputEvent::MouseMove { x: -100, y: -200 };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseMove { x, y } => {
                assert_eq!(x, -100);
                assert_eq!(y, -200);
            }
            _ => panic!("Expected MouseMove event"),
        }
    }

    #[test]
    fn test_mouse_move_max_values() {
        let event = input::InputEvent::MouseMove { 
            x: i32::MAX, 
            y: i32::MAX 
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseMove { x, y } => {
                assert_eq!(x, i32::MAX);
                assert_eq!(y, i32::MAX);
            }
            _ => panic!("Expected MouseMove event"),
        }
    }

    #[test]
    fn test_mouse_move_min_values() {
        let event = input::InputEvent::MouseMove { 
            x: i32::MIN, 
            y: i32::MIN 
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseMove { x, y } => {
                assert_eq!(x, i32::MIN);
                assert_eq!(y, i32::MIN);
            }
            _ => panic!("Expected MouseMove event"),
        }
    }

    // Test MouseButton event encoding/decoding
    #[test]
    fn test_mouse_button_all_buttons() {
        let buttons = vec![
            input::MouseButton::Left,
            input::MouseButton::Right,
            input::MouseButton::Middle,
            input::MouseButton::X1,
            input::MouseButton::X2,
        ];
        
        for button in buttons {
            for pressed in [true, false] {
                let event = input::InputEvent::MouseButton { button, pressed };
                let message = encode_input_event(&event).unwrap();
                let decoded = decode_input_event(&message).unwrap();
                
                match decoded {
                    input::InputEvent::MouseButton { button: b, pressed: p } => {
                        assert_eq!(b, button);
                        assert_eq!(p, pressed);
                    }
                    _ => panic!("Expected MouseButton event"),
                }
            }
        }
    }

    // Test MouseScroll event encoding/decoding
    #[test]
    fn test_mouse_scroll_roundtrip() {
        let event = input::InputEvent::MouseScroll { 
            delta_x: 120, 
            delta_y: -240 
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseScroll { delta_x, delta_y } => {
                assert_eq!(delta_x, 120);
                assert_eq!(delta_y, -240);
            }
            _ => panic!("Expected MouseScroll event"),
        }
    }

    #[test]
    fn test_mouse_scroll_zero_deltas() {
        let event = input::InputEvent::MouseScroll { 
            delta_x: 0, 
            delta_y: 0 
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::MouseScroll { delta_x, delta_y } => {
                assert_eq!(delta_x, 0);
                assert_eq!(delta_y, 0);
            }
            _ => panic!("Expected MouseScroll event"),
        }
    }

    // Test KeyPress event encoding/decoding
    #[test]
    fn test_key_press_simple() {
        let event = input::InputEvent::KeyPress {
            key_code: 65, // 'A'
            modifiers: input::Modifiers::default(),
            pressed: true,
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::KeyPress { key_code, modifiers, pressed } => {
                assert_eq!(key_code, 65);
                assert_eq!(modifiers, input::Modifiers::default());
                assert_eq!(pressed, true);
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    #[test]
    fn test_key_press_with_all_modifiers() {
        let event = input::InputEvent::KeyPress {
            key_code: 67, // 'C'
            modifiers: input::Modifiers {
                shift: true,
                ctrl: true,
                alt: true,
                meta: true,
            },
            pressed: true,
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::KeyPress { key_code, modifiers, pressed } => {
                assert_eq!(key_code, 67);
                assert!(modifiers.shift);
                assert!(modifiers.ctrl);
                assert!(modifiers.alt);
                assert!(modifiers.meta);
                assert_eq!(pressed, true);
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    #[test]
    fn test_key_press_with_partial_modifiers() {
        let event = input::InputEvent::KeyPress {
            key_code: 86, // 'V'
            modifiers: input::Modifiers {
                shift: false,
                ctrl: true,
                alt: false,
                meta: true,
            },
            pressed: false,
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::KeyPress { key_code, modifiers, pressed } => {
                assert_eq!(key_code, 86);
                assert!(!modifiers.shift);
                assert!(modifiers.ctrl);
                assert!(!modifiers.alt);
                assert!(modifiers.meta);
                assert_eq!(pressed, false);
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    #[test]
    fn test_key_press_max_key_code() {
        let event = input::InputEvent::KeyPress {
            key_code: u32::MAX,
            modifiers: input::Modifiers::default(),
            pressed: true,
        };
        let message = encode_input_event(&event).unwrap();
        let decoded = decode_input_event(&message).unwrap();
        
        match decoded {
            input::InputEvent::KeyPress { key_code, .. } => {
                assert_eq!(key_code, u32::MAX);
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    // Test clipboard content encoding/decoding
    #[test]
    fn test_clipboard_text_roundtrip() {
        let content = clipboard::ClipboardContent::Text("Hello, World!".to_string());
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Text(text) => {
                assert_eq!(text, "Hello, World!");
            }
            _ => panic!("Expected Text clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_empty_text() {
        let content = clipboard::ClipboardContent::Text("".to_string());
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Text(text) => {
                assert_eq!(text, "");
            }
            _ => panic!("Expected Text clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_unicode_text() {
        let content = clipboard::ClipboardContent::Text("你好世界 🌍 Привет мир".to_string());
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Text(text) => {
                assert_eq!(text, "你好世界 🌍 Привет мир");
            }
            _ => panic!("Expected Text clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_special_characters() {
        let content = clipboard::ClipboardContent::Text("\n\t\r\0".to_string());
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Text(text) => {
                assert_eq!(text, "\n\t\r\0");
            }
            _ => panic!("Expected Text clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_image_roundtrip() {
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let content = clipboard::ClipboardContent::Image(image_data.clone());
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Image(data) => {
                assert_eq!(data, image_data);
            }
            _ => panic!("Expected Image clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_empty_image() {
        let content = clipboard::ClipboardContent::Image(vec![]);
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Image(data) => {
                assert_eq!(data.len(), 0);
            }
            _ => panic!("Expected Image clipboard content"),
        }
    }

    #[test]
    fn test_clipboard_empty_roundtrip() {
        let content = clipboard::ClipboardContent::Empty;
        let message = encode_clipboard_content(&content).unwrap();
        let decoded = decode_clipboard_content(&message).unwrap();
        
        match decoded {
            clipboard::ClipboardContent::Empty => {},
            _ => panic!("Expected Empty clipboard content"),
        }
    }

    // Test full message serialization with reader/writer
    #[test]
    fn test_protocol_message_write_read() {
        let event = input::InputEvent::MouseMove { x: 500, y: 600 };
        let message = encode_input_event(&event).unwrap();
        
        let mut buffer = Vec::new();
        message.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let read_message = ProtocolMessage::from_reader(&mut cursor).unwrap();
        
        assert_eq!(read_message.header.event_type, message.header.event_type);
        assert_eq!(read_message.payload, message.payload);
    }

    #[test]
    fn test_protocol_message_to_bytes() {
        let payload = vec![1, 2, 3, 4];
        let message = ProtocolMessage::new(EventType::Heartbeat, payload.clone());
        let bytes = message.to_bytes();
        
        assert_eq!(bytes.len(), 8 + payload.len());
        assert_eq!(&bytes[8..], &payload[..]);
    }

    // Test edge cases
    #[test]
    fn test_empty_payload() {
        let message = ProtocolMessage::new(EventType::Heartbeat, vec![]);
        let bytes = message.to_bytes();
        
        assert_eq!(bytes.len(), 8);
        
        let mut cursor = Cursor::new(bytes);
        let read_message = ProtocolMessage::from_reader(&mut cursor).unwrap();
        
        assert_eq!(read_message.payload.len(), 0);
    }

    #[test]
    fn test_large_payload() {
        let large_payload = vec![0xAB; 10000];
        let message = ProtocolMessage::new(EventType::ClipboardSync, large_payload.clone());
        
        let mut buffer = Vec::new();
        message.write_to(&mut buffer).unwrap();
        
        let mut cursor = Cursor::new(buffer);
        let read_message = ProtocolMessage::from_reader(&mut cursor).unwrap();
        
        assert_eq!(read_message.payload, large_payload);
    }

    #[test]
    fn test_decode_wrong_event_type() {
        // Create a MouseMove message but try to decode as clipboard
        let event = input::InputEvent::MouseMove { x: 100, y: 200 };
        let message = encode_input_event(&event).unwrap();
        
        let result = decode_clipboard_content(&message);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_payload() {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&MAGIC_NUMBER.to_be_bytes());
        bytes[2] = PROTOCOL_VERSION;
        bytes[3] = EventType::MouseMove as u8;
        bytes[4..8].copy_from_slice(&100u32.to_be_bytes()); // Claims 100 bytes payload
        
        let mut cursor = Cursor::new(bytes.to_vec());
        let result = ProtocolMessage::from_reader(&mut cursor);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod data_model_tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_device_info_creation() {
        let device_info = device::DeviceInfo {
            id: "device-1".to_string(),
            name: "Test Device".to_string(),
            ip_address: IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)),
            port: 5000,
            os_type: device::OsType::Windows,
            public_key: vec![1, 2, 3, 4],
        };
        
        assert_eq!(device_info.id, "device-1");
        assert_eq!(device_info.name, "Test Device");
        assert_eq!(device_info.port, 5000);
    }

    #[test]
    fn test_device_status_variants() {
        let statuses = vec![
            device::DeviceStatus::Online,
            device::DeviceStatus::Offline,
            device::DeviceStatus::Connecting,
        ];
        
        for status in statuses {
            let device = device::Device {
                id: "test".to_string(),
                name: "Test".to_string(),
                os_type: device::OsType::MacOS,
                ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                port: 5000,
                public_key: vec![],
                screen_resolution: (1920, 1080),
                status,
            };
            
            assert_eq!(device.status, status);
        }
    }

    #[test]
    fn test_modifiers_default() {
        let modifiers = input::Modifiers::default();
        assert!(!modifiers.shift);
        assert!(!modifiers.ctrl);
        assert!(!modifiers.alt);
        assert!(!modifiers.meta);
    }

    #[test]
    fn test_preferences_default() {
        let prefs = config::Preferences::default();
        assert_eq!(prefs.edge_switch_delay_ms, 200);
        assert_eq!(prefs.clipboard_sync_enabled, true);
        assert_eq!(prefs.clipboard_size_limit_mb, 10);
        assert_eq!(prefs.show_notifications, true);
        assert_eq!(prefs.network_timeout_ms, 5000);
    }

    #[test]
    fn test_device_position() {
        let pos = config::DevicePosition {
            x: 100,
            y: 200,
            width: 1920,
            height: 1080,
        };
        
        assert_eq!(pos.x, 100);
        assert_eq!(pos.y, 200);
        assert_eq!(pos.width, 1920);
        assert_eq!(pos.height, 1080);
    }
}
