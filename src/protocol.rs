use crate::{Result, KvmError};
use crate::input::{InputEvent, MouseButton, Modifiers};
use crate::clipboard::ClipboardContent;
use std::io::{Read, Write};

/// Magic number for protocol identification: "KV" in ASCII
pub const MAGIC_NUMBER: u16 = 0x4B56;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 0x01;

/// Event types in the binary protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    MouseMove = 0x01,
    MouseButton = 0x02,
    MouseScroll = 0x03,
    KeyPress = 0x04,
    ClipboardSync = 0x05,
    SwitchRequest = 0x06,
    Heartbeat = 0x07,
}

impl EventType {
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(EventType::MouseMove),
            0x02 => Ok(EventType::MouseButton),
            0x03 => Ok(EventType::MouseScroll),
            0x04 => Ok(EventType::KeyPress),
            0x05 => Ok(EventType::ClipboardSync),
            0x06 => Ok(EventType::SwitchRequest),
            0x07 => Ok(EventType::Heartbeat),
            _ => Err(KvmError::Protocol(format!("Invalid event type: {}", value))),
        }
    }
}

/// Protocol message header (8 bytes)
#[derive(Debug, Clone, Copy)]
pub struct MessageHeader {
    pub magic: u16,
    pub version: u8,
    pub event_type: EventType,
    pub payload_length: u32,
}

impl MessageHeader {
    pub fn new(event_type: EventType, payload_length: u32) -> Self {
        Self {
            magic: MAGIC_NUMBER,
            version: PROTOCOL_VERSION,
            event_type,
            payload_length,
        }
    }

    /// Serialize header to bytes (8 bytes total)
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&self.magic.to_be_bytes());
        bytes[2] = self.version;
        bytes[3] = self.event_type as u8;
        bytes[4..8].copy_from_slice(&self.payload_length.to_be_bytes());
        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(KvmError::Protocol("Header too short".to_string()));
        }

        let magic = u16::from_be_bytes([bytes[0], bytes[1]]);
        if magic != MAGIC_NUMBER {
            return Err(KvmError::Protocol(format!("Invalid magic number: 0x{:04X}", magic)));
        }

        let version = bytes[2];
        if version != PROTOCOL_VERSION {
            return Err(KvmError::Protocol(format!("Unsupported protocol version: {}", version)));
        }

        let event_type = EventType::from_u8(bytes[3])?;
        let payload_length = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Ok(Self {
            magic,
            version,
            event_type,
            payload_length,
        })
    }
}

/// Protocol message containing header and payload
#[derive(Debug, Clone)]
pub struct ProtocolMessage {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
}

impl ProtocolMessage {
    pub fn new(event_type: EventType, payload: Vec<u8>) -> Self {
        let header = MessageHeader::new(event_type, payload.len() as u32);
        Self { header, payload }
    }

    /// Serialize the entire message to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.payload.len());
        bytes.extend_from_slice(&self.header.to_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Deserialize a message from a reader
    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut header_bytes = [0u8; 8];
        reader.read_exact(&mut header_bytes).map_err(|e| {
            KvmError::Protocol(format!("Failed to read header: {}", e))
        })?;

        let header = MessageHeader::from_bytes(&header_bytes)?;

        let mut payload = vec![0u8; header.payload_length as usize];
        reader.read_exact(&mut payload).map_err(|e| {
            KvmError::Protocol(format!("Failed to read payload: {}", e))
        })?;

        Ok(Self { header, payload })
    }

    /// Write the message to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.header.to_bytes()).map_err(|e| {
            KvmError::Protocol(format!("Failed to write header: {}", e))
        })?;
        writer.write_all(&self.payload).map_err(|e| {
            KvmError::Protocol(format!("Failed to write payload: {}", e))
        })?;
        Ok(())
    }
}

/// Encode an InputEvent into a protocol message
pub fn encode_input_event(event: &InputEvent) -> Result<ProtocolMessage> {
    match event {
        InputEvent::MouseMove { x, y } => {
            let mut payload = Vec::with_capacity(8);
            payload.extend_from_slice(&x.to_be_bytes());
            payload.extend_from_slice(&y.to_be_bytes());
            Ok(ProtocolMessage::new(EventType::MouseMove, payload))
        }
        InputEvent::MouseButton { button, pressed } => {
            let button_code = match button {
                MouseButton::Left => 0u8,
                MouseButton::Right => 1u8,
                MouseButton::Middle => 2u8,
                MouseButton::X1 => 3u8,
                MouseButton::X2 => 4u8,
            };
            let pressed_byte = if *pressed { 1u8 } else { 0u8 };
            let payload = vec![button_code, pressed_byte];
            Ok(ProtocolMessage::new(EventType::MouseButton, payload))
        }
        InputEvent::MouseScroll { delta_x, delta_y } => {
            let mut payload = Vec::with_capacity(8);
            payload.extend_from_slice(&delta_x.to_be_bytes());
            payload.extend_from_slice(&delta_y.to_be_bytes());
            Ok(ProtocolMessage::new(EventType::MouseScroll, payload))
        }
        InputEvent::KeyPress { key_code, modifiers, pressed } => {
            let mut payload = Vec::with_capacity(6);
            payload.extend_from_slice(&key_code.to_be_bytes());
            let modifiers_byte = encode_modifiers(modifiers);
            payload.push(modifiers_byte);
            payload.push(if *pressed { 1u8 } else { 0u8 });
            Ok(ProtocolMessage::new(EventType::KeyPress, payload))
        }
    }
}

/// Decode a protocol message into an InputEvent
pub fn decode_input_event(message: &ProtocolMessage) -> Result<InputEvent> {
    match message.header.event_type {
        EventType::MouseMove => {
            if message.payload.len() < 8 {
                return Err(KvmError::Protocol("MouseMove payload too short".to_string()));
            }
            let x = i32::from_be_bytes([
                message.payload[0],
                message.payload[1],
                message.payload[2],
                message.payload[3],
            ]);
            let y = i32::from_be_bytes([
                message.payload[4],
                message.payload[5],
                message.payload[6],
                message.payload[7],
            ]);
            Ok(InputEvent::MouseMove { x, y })
        }
        EventType::MouseButton => {
            if message.payload.len() < 2 {
                return Err(KvmError::Protocol("MouseButton payload too short".to_string()));
            }
            let button = match message.payload[0] {
                0 => MouseButton::Left,
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                3 => MouseButton::X1,
                4 => MouseButton::X2,
                _ => return Err(KvmError::Protocol(format!("Invalid mouse button: {}", message.payload[0]))),
            };
            let pressed = message.payload[1] != 0;
            Ok(InputEvent::MouseButton { button, pressed })
        }
        EventType::MouseScroll => {
            if message.payload.len() < 8 {
                return Err(KvmError::Protocol("MouseScroll payload too short".to_string()));
            }
            let delta_x = i32::from_be_bytes([
                message.payload[0],
                message.payload[1],
                message.payload[2],
                message.payload[3],
            ]);
            let delta_y = i32::from_be_bytes([
                message.payload[4],
                message.payload[5],
                message.payload[6],
                message.payload[7],
            ]);
            Ok(InputEvent::MouseScroll { delta_x, delta_y })
        }
        EventType::KeyPress => {
            if message.payload.len() < 6 {
                return Err(KvmError::Protocol("KeyPress payload too short".to_string()));
            }
            let key_code = u32::from_be_bytes([
                message.payload[0],
                message.payload[1],
                message.payload[2],
                message.payload[3],
            ]);
            let modifiers = decode_modifiers(message.payload[4]);
            let pressed = message.payload[5] != 0;
            Ok(InputEvent::KeyPress { key_code, modifiers, pressed })
        }
        _ => Err(KvmError::Protocol(format!("Cannot decode event type {:?} as InputEvent", message.header.event_type))),
    }
}

/// Encode modifiers into a single byte
/// Bit layout: [unused:4][meta:1][alt:1][ctrl:1][shift:1]
fn encode_modifiers(modifiers: &Modifiers) -> u8 {
    let mut byte = 0u8;
    if modifiers.shift { byte |= 0b0001; }
    if modifiers.ctrl { byte |= 0b0010; }
    if modifiers.alt { byte |= 0b0100; }
    if modifiers.meta { byte |= 0b1000; }
    byte
}

/// Decode modifiers from a single byte
fn decode_modifiers(byte: u8) -> Modifiers {
    Modifiers {
        shift: (byte & 0b0001) != 0,
        ctrl: (byte & 0b0010) != 0,
        alt: (byte & 0b0100) != 0,
        meta: (byte & 0b1000) != 0,
    }
}

/// Encode clipboard content into a protocol message
pub fn encode_clipboard_content(content: &ClipboardContent) -> Result<ProtocolMessage> {
    let payload = match content {
        ClipboardContent::Text(text) => {
            let mut payload = Vec::with_capacity(1 + text.len());
            payload.push(0u8); // Type: Text
            payload.extend_from_slice(text.as_bytes());
            payload
        }
        ClipboardContent::Image(data) => {
            let mut payload = Vec::with_capacity(1 + data.len());
            payload.push(1u8); // Type: Image
            payload.extend_from_slice(data);
            payload
        }
        ClipboardContent::Empty => {
            vec![2u8] // Type: Empty
        }
    };
    Ok(ProtocolMessage::new(EventType::ClipboardSync, payload))
}

/// Decode clipboard content from a protocol message
pub fn decode_clipboard_content(message: &ProtocolMessage) -> Result<ClipboardContent> {
    if message.header.event_type != EventType::ClipboardSync {
        return Err(KvmError::Protocol("Message is not a ClipboardSync event".to_string()));
    }

    if message.payload.is_empty() {
        return Err(KvmError::Protocol("ClipboardSync payload is empty".to_string()));
    }

    match message.payload[0] {
        0 => {
            // Text
            let text = String::from_utf8(message.payload[1..].to_vec())
                .map_err(|e| KvmError::Protocol(format!("Invalid UTF-8 in clipboard text: {}", e)))?;
            Ok(ClipboardContent::Text(text))
        }
        1 => {
            // Image
            Ok(ClipboardContent::Image(message.payload[1..].to_vec()))
        }
        2 => {
            // Empty
            Ok(ClipboardContent::Empty)
        }
        _ => Err(KvmError::Protocol(format!("Invalid clipboard content type: {}", message.payload[0]))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header_roundtrip() {
        let header = MessageHeader::new(EventType::MouseMove, 1234);
        let bytes = header.to_bytes();
        let decoded = MessageHeader::from_bytes(&bytes).unwrap();
        
        assert_eq!(decoded.magic, MAGIC_NUMBER);
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.event_type, EventType::MouseMove);
        assert_eq!(decoded.payload_length, 1234);
    }

    #[test]
    fn test_invalid_magic_number() {
        let mut bytes = [0u8; 8];
        bytes[0..2].copy_from_slice(&0xFFFFu16.to_be_bytes());
        
        let result = MessageHeader::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_modifiers_encoding() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        
        let encoded = encode_modifiers(&modifiers);
        let decoded = decode_modifiers(encoded);
        
        assert_eq!(modifiers, decoded);
    }
}
