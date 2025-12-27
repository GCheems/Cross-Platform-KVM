use crate::input::InputEvent;
use crate::protocol::ProtocolMessage;
use crate::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Configuration for input event batching
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of events in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before sending a batch (milliseconds)
    pub max_batch_delay_ms: u64,
    /// Whether to enable batching
    pub enabled: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 10,
            max_batch_delay_ms: 5, // 5ms to keep latency low
            enabled: true,
        }
    }
}

/// Input event batcher that accumulates events and sends them in batches
pub struct InputEventBatcher {
    config: BatchConfig,
    batch: Arc<Mutex<Vec<InputEvent>>>,
    last_flush: Arc<Mutex<Instant>>,
}

impl InputEventBatcher {
    /// Create a new input event batcher
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            batch: Arc::new(Mutex::new(Vec::new())),
            last_flush: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Add an event to the batch
    /// Returns Some(batch) if the batch should be flushed, None otherwise
    pub async fn add_event(&self, event: InputEvent) -> Option<Vec<InputEvent>> {
        if !self.config.enabled {
            // If batching is disabled, return immediately
            return Some(vec![event]);
        }

        let mut batch = self.batch.lock().await;
        batch.push(event);

        // Check if we should flush
        let should_flush = batch.len() >= self.config.max_batch_size
            || self.should_flush_by_time().await;

        if should_flush {
            let events = batch.drain(..).collect();
            *self.last_flush.lock().await = Instant::now();
            Some(events)
        } else {
            None
        }
    }

    /// Check if enough time has passed to flush the batch
    async fn should_flush_by_time(&self) -> bool {
        let last_flush = self.last_flush.lock().await;
        last_flush.elapsed() >= Duration::from_millis(self.config.max_batch_delay_ms)
    }

    /// Force flush the current batch
    pub async fn flush(&self) -> Vec<InputEvent> {
        let mut batch = self.batch.lock().await;
        let events = batch.drain(..).collect();
        *self.last_flush.lock().await = Instant::now();
        events
    }

    /// Get the current batch size
    pub async fn batch_size(&self) -> usize {
        self.batch.lock().await.len()
    }
}

/// Batch multiple protocol messages into a single message
pub fn batch_protocol_messages(messages: Vec<ProtocolMessage>) -> Result<Vec<u8>> {
    let mut batched = Vec::new();
    
    // Write the number of messages
    batched.extend_from_slice(&(messages.len() as u32).to_be_bytes());
    
    // Write each message
    for message in messages {
        let bytes = message.to_bytes();
        batched.extend_from_slice(&bytes);
    }
    
    Ok(batched)
}

/// Unbatch protocol messages from a batched payload
pub fn unbatch_protocol_messages(data: &[u8]) -> Result<Vec<ProtocolMessage>> {
    use std::io::Cursor;
    use crate::protocol::ProtocolMessage;
    
    if data.len() < 4 {
        return Err(crate::KvmError::Protocol("Batched data too short".to_string()));
    }
    
    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut messages = Vec::with_capacity(count);
    
    let mut cursor = Cursor::new(&data[4..]);
    for _ in 0..count {
        let message = ProtocolMessage::from_reader(&mut cursor)?;
        messages.push(message);
    }
    
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{MouseButton, Modifiers};

    #[tokio::test]
    async fn test_batcher_immediate_flush_when_disabled() {
        let config = BatchConfig {
            enabled: false,
            ..Default::default()
        };
        let batcher = InputEventBatcher::new(config);
        
        let event = InputEvent::MouseMove { x: 100, y: 200 };
        let result = batcher.add_event(event.clone()).await;
        
        assert!(result.is_some());
        let batch = result.unwrap();
        assert_eq!(batch.len(), 1);
    }

    #[tokio::test]
    async fn test_batcher_accumulates_events() {
        let config = BatchConfig {
            max_batch_size: 5,
            max_batch_delay_ms: 1000,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);
        
        // Add 4 events (below threshold)
        for i in 0..4 {
            let event = InputEvent::MouseMove { x: i, y: i };
            let result = batcher.add_event(event).await;
            assert!(result.is_none());
        }
        
        assert_eq!(batcher.batch_size().await, 4);
    }

    #[tokio::test]
    async fn test_batcher_flushes_on_size() {
        let config = BatchConfig {
            max_batch_size: 3,
            max_batch_delay_ms: 1000,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);
        
        // Add 2 events
        for i in 0..2 {
            let event = InputEvent::MouseMove { x: i, y: i };
            let result = batcher.add_event(event).await;
            assert!(result.is_none());
        }
        
        // Add 3rd event - should trigger flush
        let event = InputEvent::MouseMove { x: 2, y: 2 };
        let result = batcher.add_event(event).await;
        
        assert!(result.is_some());
        let batch = result.unwrap();
        assert_eq!(batch.len(), 3);
        assert_eq!(batcher.batch_size().await, 0);
    }

    #[tokio::test]
    async fn test_batcher_flushes_on_time() {
        let config = BatchConfig {
            max_batch_size: 100,
            max_batch_delay_ms: 10, // 10ms
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);
        
        // Add an event
        let event = InputEvent::MouseMove { x: 0, y: 0 };
        let result = batcher.add_event(event).await;
        assert!(result.is_none());
        
        // Wait for the delay
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        // Add another event - should trigger flush due to time
        let event = InputEvent::MouseMove { x: 1, y: 1 };
        let result = batcher.add_event(event).await;
        
        assert!(result.is_some());
        let batch = result.unwrap();
        assert_eq!(batch.len(), 2);
    }

    #[tokio::test]
    async fn test_manual_flush() {
        let config = BatchConfig::default();
        let batcher = InputEventBatcher::new(config);
        
        // Add some events
        for i in 0..3 {
            let event = InputEvent::MouseMove { x: i, y: i };
            let _ = batcher.add_event(event).await;
        }
        
        // Manual flush
        let batch = batcher.flush().await;
        assert_eq!(batch.len(), 3);
        assert_eq!(batcher.batch_size().await, 0);
    }

    #[test]
    fn test_batch_protocol_messages() {
        use crate::protocol::{encode_input_event, EventType};
        
        let events = vec![
            InputEvent::MouseMove { x: 100, y: 200 },
            InputEvent::MouseButton { button: MouseButton::Left, pressed: true },
            InputEvent::KeyPress { key_code: 65, modifiers: Modifiers::default(), pressed: true },
        ];
        
        let messages: Vec<_> = events.iter()
            .map(|e| encode_input_event(e).unwrap())
            .collect();
        
        let batched = batch_protocol_messages(messages.clone()).unwrap();
        let unbatched = unbatch_protocol_messages(&batched).unwrap();
        
        assert_eq!(unbatched.len(), 3);
        assert_eq!(unbatched[0].header.event_type, EventType::MouseMove);
        assert_eq!(unbatched[1].header.event_type, EventType::MouseButton);
        assert_eq!(unbatched[2].header.event_type, EventType::KeyPress);
    }
}
