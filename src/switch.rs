use crate::{Result, KvmError};
use crate::config::{DeviceLayout, DevicePosition};
use crate::device::Device;
use crate::input::{InputEvent, InputInjection};
use crate::protocol::{encode_input_event, ProtocolMessage};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::io::AsyncWriteExt;

/// Device switch event
#[derive(Debug, Clone)]
pub enum SwitchEvent {
    SwitchedTo(String),
    SwitchFailed(String, String), // device_id, error_message
}

/// Trait for switch controller
/// Manages device switching logic
#[async_trait::async_trait]
pub trait SwitchController: Send + Sync {
    /// Switch to the specified device
    async fn switch_to(&self, device_id: &str) -> Result<()>;

    /// Check if edge switch should be triggered at the given position
    /// Returns the target device ID if a switch should occur
    fn should_trigger_edge_switch(&self, x: i32, y: i32) -> Option<String>;

    /// Get the currently active device ID
    fn get_active_device(&self) -> Option<String>;

    /// Subscribe to switch events
    fn subscribe(&self) -> mpsc::Receiver<SwitchEvent>;
}

/// Edge direction for screen edge detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Left,
    Right,
    Top,
    Bottom,
}

/// Edge detection state
#[derive(Debug, Clone)]
struct EdgeState {
    /// Position where edge was first detected
    position: (i32, i32),
    /// Time when edge was first detected
    first_detected: Instant,
    /// Edge direction
    direction: EdgeDirection,
}

/// Implementation of the switch controller
pub struct DefaultSwitchController {
    /// Currently active device ID
    active_device: Arc<RwLock<Option<String>>>,
    /// Device layout configuration
    layout: Arc<RwLock<DeviceLayout>>,
    /// Available devices with their screen resolutions
    devices: Arc<RwLock<HashMap<String, Device>>>,
    /// Edge detection delay in milliseconds
    edge_delay_ms: u64,
    /// Current edge detection state
    edge_state: Arc<RwLock<Option<EdgeState>>>,
    /// Event broadcaster
    event_tx: mpsc::Sender<SwitchEvent>,
    /// Event receiver (for subscription)
    event_rx: Arc<RwLock<Option<mpsc::Receiver<SwitchEvent>>>>,
}

impl DefaultSwitchController {
    /// Create a new switch controller
    pub fn new(edge_delay_ms: u64) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        Self {
            active_device: Arc::new(RwLock::new(None)),
            layout: Arc::new(RwLock::new(DeviceLayout {
                devices: HashMap::new(),
            })),
            devices: Arc::new(RwLock::new(HashMap::new())),
            edge_delay_ms,
            edge_state: Arc::new(RwLock::new(None)),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }

    /// Update the device layout
    pub async fn update_layout(&self, layout: DeviceLayout) {
        let mut current_layout = self.layout.write().await;
        *current_layout = layout;
    }

    /// Register a device with its screen resolution
    pub async fn register_device(&self, device: Device) {
        let mut devices = self.devices.write().await;
        devices.insert(device.id.clone(), device);
    }

    /// Unregister a device
    pub async fn unregister_device(&self, device_id: &str) {
        let mut devices = self.devices.write().await;
        devices.remove(device_id);
    }

    /// Get the screen resolution of the active device
    pub async fn get_active_resolution(&self) -> Option<(u32, u32)> {
        let active = self.active_device.read().await;
        if let Some(device_id) = active.as_ref() {
            let devices = self.devices.read().await;
            devices.get(device_id).map(|d| d.screen_resolution)
        } else {
            None
        }
    }

    /// Detect which edge (if any) the position is on
    fn detect_edge(&self, x: i32, y: i32, width: i32, height: i32) -> Option<EdgeDirection> {
        const EDGE_THRESHOLD: i32 = 5; // pixels from edge
        
        if x <= EDGE_THRESHOLD {
            Some(EdgeDirection::Left)
        } else if x >= width - EDGE_THRESHOLD {
            Some(EdgeDirection::Right)
        } else if y <= EDGE_THRESHOLD {
            Some(EdgeDirection::Top)
        } else if y >= height - EDGE_THRESHOLD {
            Some(EdgeDirection::Bottom)
        } else {
            None
        }
    }

    /// Find the adjacent device in the given direction
    async fn find_adjacent_device(&self, direction: EdgeDirection) -> Option<String> {
        let active = self.active_device.read().await;
        let active_id = active.as_ref()?;
        
        let layout = self.layout.read().await;
        let active_pos = layout.devices.get(active_id)?;
        
        // Find device in the specified direction
        let mut best_match: Option<(String, i32)> = None;
        
        for (device_id, pos) in &layout.devices {
            if device_id == active_id {
                continue;
            }
            
            let is_adjacent = match direction {
                EdgeDirection::Left => {
                    // Device should be to the left (pos.x + pos.width <= active_pos.x)
                    // and vertically overlapping
                    pos.x + pos.width <= active_pos.x &&
                    !(pos.y + pos.height <= active_pos.y || pos.y >= active_pos.y + active_pos.height)
                }
                EdgeDirection::Right => {
                    // Device should be to the right (pos.x >= active_pos.x + active_pos.width)
                    // and vertically overlapping
                    pos.x >= active_pos.x + active_pos.width &&
                    !(pos.y + pos.height <= active_pos.y || pos.y >= active_pos.y + active_pos.height)
                }
                EdgeDirection::Top => {
                    // Device should be above (pos.y + pos.height <= active_pos.y)
                    // and horizontally overlapping
                    pos.y + pos.height <= active_pos.y &&
                    !(pos.x + pos.width <= active_pos.x || pos.x >= active_pos.x + active_pos.width)
                }
                EdgeDirection::Bottom => {
                    // Device should be below (pos.y >= active_pos.y + active_pos.height)
                    // and horizontally overlapping
                    pos.y >= active_pos.y + active_pos.height &&
                    !(pos.x + pos.width <= active_pos.x || pos.x >= active_pos.x + active_pos.width)
                }
            };
            
            if is_adjacent {
                // Calculate distance to find the closest device
                let distance = match direction {
                    EdgeDirection::Left => active_pos.x - (pos.x + pos.width),
                    EdgeDirection::Right => pos.x - (active_pos.x + active_pos.width),
                    EdgeDirection::Top => active_pos.y - (pos.y + pos.height),
                    EdgeDirection::Bottom => pos.y - (active_pos.y + active_pos.height),
                };
                
                if let Some((_, best_distance)) = &best_match {
                    if distance < *best_distance {
                        best_match = Some((device_id.clone(), distance));
                    }
                } else {
                    best_match = Some((device_id.clone(), distance));
                }
            }
        }
        
        best_match.map(|(id, _)| id)
    }

    /// Map mouse position from source device to target device
    /// Returns the mapped position on the target device
    pub async fn map_mouse_position(
        &self,
        source_device_id: &str,
        target_device_id: &str,
        source_x: i32,
        source_y: i32,
        direction: EdgeDirection,
    ) -> Option<(i32, i32)> {
        let layout = self.layout.read().await;
        let devices = self.devices.read().await;
        
        let source_pos = layout.devices.get(source_device_id)?;
        let target_pos = layout.devices.get(target_device_id)?;
        
        let source_device = devices.get(source_device_id)?;
        let target_device = devices.get(target_device_id)?;
        
        let source_res = source_device.screen_resolution;
        let target_res = target_device.screen_resolution;
        
        // Calculate relative position on source device (0.0 to 1.0)
        let (rel_x, rel_y) = match direction {
            EdgeDirection::Left | EdgeDirection::Right => {
                // Preserve vertical position, map to edge
                let rel_y = source_y as f64 / source_res.1 as f64;
                let rel_x = if direction == EdgeDirection::Left { 1.0 } else { 0.0 };
                (rel_x, rel_y)
            }
            EdgeDirection::Top | EdgeDirection::Bottom => {
                // Preserve horizontal position, map to edge
                let rel_x = source_x as f64 / source_res.0 as f64;
                let rel_y = if direction == EdgeDirection::Top { 1.0 } else { 0.0 };
                (rel_x, rel_y)
            }
        };
        
        // Map to target device coordinates
        let target_x = (rel_x * target_res.0 as f64) as i32;
        let target_y = (rel_y * target_res.1 as f64) as i32;
        
        // Clamp to target device bounds
        let target_x = target_x.max(0).min(target_res.0 as i32 - 1);
        let target_y = target_y.max(0).min(target_res.1 as i32 - 1);
        
        Some((target_x, target_y))
    }
}

#[async_trait::async_trait]
impl SwitchController for DefaultSwitchController {
    async fn switch_to(&self, device_id: &str) -> Result<()> {
        // Verify device exists
        let devices = self.devices.read().await;
        if !devices.contains_key(device_id) {
            return Err(KvmError::DeviceNotFound(device_id.to_string()));
        }
        drop(devices);
        
        // Update active device
        let mut active = self.active_device.write().await;
        *active = Some(device_id.to_string());
        drop(active);
        
        // Clear edge state
        let mut edge_state = self.edge_state.write().await;
        *edge_state = None;
        drop(edge_state);
        
        // Send event
        let _ = self.event_tx.send(SwitchEvent::SwitchedTo(device_id.to_string())).await;
        
        Ok(())
    }

    /// Check if edge switch should be triggered at the given position
    /// Returns the target device ID if a switch should occur
    /// Note: This is a synchronous method for compatibility with existing code
    /// Consider using check_edge_switch_async for better async integration
    fn should_trigger_edge_switch(&self, x: i32, y: i32) -> Option<String> {
        // Use try_read to avoid blocking
        let active = self.active_device.try_read().ok()?;
        let active_id = active.as_ref()?.clone(); // Clone to avoid borrow issues
        drop(active);
        
        let devices = self.devices.try_read().ok()?;
        let active_device = devices.get(&active_id)?;
        let resolution = active_device.screen_resolution;
        drop(devices);
        
        // Detect if we're at an edge
        let edge = self.detect_edge(x, y, resolution.0 as i32, resolution.1 as i32)?;
        
        // Check edge state
        let mut edge_state = self.edge_state.try_write().ok()?;
        
        let now = Instant::now();
        
        match edge_state.as_ref() {
            Some(state) => {
                // Check if we're still at the same edge
                if state.direction == edge {
                    // Check if enough time has passed
                    let elapsed = now.duration_since(state.first_detected);
                    if elapsed >= Duration::from_millis(self.edge_delay_ms) {
                        // Trigger switch - but we can't call async function here
                        // So we'll use a synchronous helper function
                        let layout = self.layout.try_read().ok()?;
                        let target = DefaultSwitchController::find_adjacent_device_sync(&layout, &active_id, edge)?;
                        drop(layout);
                        
                        // Clear edge state
                        *edge_state = None;
                        
                        return Some(target);
                    }
                } else {
                    // Different edge, reset state
                    *edge_state = Some(EdgeState {
                        direction: edge,
                        first_detected: now,
                        position: (x, y),
                    });
                }
            }
            None => {
                // First time at edge, record it
                *edge_state = Some(EdgeState {
                    direction: edge,
                    first_detected: now,
                    position: (x, y),
                });
            }
        }
        
        None
    }

    fn get_active_device(&self) -> Option<String> {
        self.active_device.try_read().ok()?.clone()
    }

    fn subscribe(&self) -> mpsc::Receiver<SwitchEvent> {
        // Take the receiver if available, otherwise create a new channel
        let mut rx_lock = self.event_rx.try_write().expect("Failed to get event receiver");
        if let Some(rx) = rx_lock.take() {
            rx
        } else {
            // Create a new receiver (this shouldn't happen in normal usage)
            let (_, rx) = mpsc::channel(100);
            rx
        }
    }
}

// Private helper methods for DefaultSwitchController
impl DefaultSwitchController {
    /// Synchronous version of find_adjacent_device for use in should_trigger_edge_switch
    fn find_adjacent_device_sync(
        layout: &DeviceLayout,
        active_id: &str,
        direction: EdgeDirection,
    ) -> Option<String> {
        let active_pos = layout.devices.get(active_id)?;
        
        let mut best_match: Option<(String, i32)> = None;
        
        for (device_id, pos) in &layout.devices {
            if device_id == active_id {
                continue;
            }
            
            let is_adjacent = match direction {
                EdgeDirection::Left => {
                    pos.x + pos.width <= active_pos.x &&
                    !(pos.y + pos.height <= active_pos.y || pos.y >= active_pos.y + active_pos.height)
                }
                EdgeDirection::Right => {
                    pos.x >= active_pos.x + active_pos.width &&
                    !(pos.y + pos.height <= active_pos.y || pos.y >= active_pos.y + active_pos.height)
                }
                EdgeDirection::Top => {
                    pos.y + pos.height <= active_pos.y &&
                    !(pos.x + pos.width <= active_pos.x || pos.x >= active_pos.x + active_pos.width)
                }
                EdgeDirection::Bottom => {
                    pos.y >= active_pos.y + active_pos.height &&
                    !(pos.x + pos.width <= active_pos.x || pos.x >= active_pos.x + active_pos.width)
                }
            };
            
            if is_adjacent {
                let distance = match direction {
                    EdgeDirection::Left => active_pos.x - (pos.x + pos.width),
                    EdgeDirection::Right => pos.x - (active_pos.x + active_pos.width),
                    EdgeDirection::Top => active_pos.y - (pos.y + pos.height),
                    EdgeDirection::Bottom => pos.y - (active_pos.y + active_pos.height),
                };
                
                if let Some((_, best_distance)) = &best_match {
                    if distance < *best_distance {
                        best_match = Some((device_id.clone(), distance));
                    }
                } else {
                    best_match = Some((device_id.clone(), distance));
                }
            }
        }
        
        best_match.map(|(id, _)| id)
    }
}

/// Input router trait for routing keyboard and mouse input to the active device
#[async_trait::async_trait]
pub trait InputRouter: Send + Sync {
    /// Route an input event to the currently active device
    /// Returns the device ID that received the event
    async fn route_input(&self, event: InputEvent) -> Result<String>;
    
    /// Get the current routing target (active device)
    fn get_routing_target(&self) -> Option<String>;
    
    /// Set the routing target (called when device switches)
    async fn set_routing_target(&self, device_id: Option<String>) -> Result<()>;
}

/// Input latency event for monitoring
#[derive(Debug, Clone)]
pub struct InputLatencyEvent {
    pub device_id: String,
    pub latency_ms: u64,
    pub event_type: String,
    pub timestamp: Instant,
}

/// Default implementation of input router
pub struct DefaultInputRouter {
    /// Currently active device (routing target)
    active_device: Arc<RwLock<Option<String>>>,
    
    /// Input injection service for local injection
    local_injector: Arc<dyn InputInjection>,
    
    /// Network connections for remote injection (device_id -> writer)
    /// In a real implementation, this would be integrated with the network module
    connections: Arc<RwLock<HashMap<String, mpsc::Sender<ProtocolMessage>>>>,
    
    /// Latency monitoring channel
    latency_tx: mpsc::Sender<InputLatencyEvent>,
    latency_rx: Arc<RwLock<Option<mpsc::Receiver<InputLatencyEvent>>>>,
    
    /// Local device ID
    local_device_id: String,
}

impl DefaultInputRouter {
    /// Create a new input router
    pub fn new(local_device_id: String, local_injector: Arc<dyn InputInjection>) -> Self {
        let (latency_tx, latency_rx) = mpsc::channel(100);
        
        Self {
            active_device: Arc::new(RwLock::new(None)),
            local_injector,
            connections: Arc::new(RwLock::new(HashMap::new())),
            latency_tx,
            latency_rx: Arc::new(RwLock::new(Some(latency_rx))),
            local_device_id,
        }
    }
    
    /// Register a network connection for a remote device
    pub async fn register_connection(&self, device_id: String, sender: mpsc::Sender<ProtocolMessage>) {
        let mut connections = self.connections.write().await;
        connections.insert(device_id, sender);
    }
    
    /// Unregister a network connection
    pub async fn unregister_connection(&self, device_id: &str) {
        let mut connections = self.connections.write().await;
        connections.remove(device_id);
    }
    
    /// Subscribe to latency monitoring events
    pub fn subscribe_latency(&self) -> Option<mpsc::Receiver<InputLatencyEvent>> {
        let mut rx_opt = futures::executor::block_on(self.latency_rx.write());
        rx_opt.take()
    }
    
    /// Route input to a remote device over the network
    async fn route_to_remote(&self, device_id: &str, event: InputEvent) -> Result<()> {
        let start_time = Instant::now();
        
        // Encode the input event
        let message = encode_input_event(&event)?;
        
        // Get the connection for this device
        let connections = self.connections.read().await;
        let sender = connections.get(device_id)
            .ok_or_else(|| KvmError::Connection(format!("No connection to device {}", device_id)))?;
        
        // Send the message
        sender.send(message).await
            .map_err(|e| KvmError::Connection(format!("Failed to send input event: {}", e)))?;
        
        // Calculate latency
        let latency = start_time.elapsed();
        let latency_ms = latency.as_millis() as u64;
        
        // Record latency event
        let event_type = match event {
            InputEvent::MouseMove { .. } => "MouseMove",
            InputEvent::MouseButton { .. } => "MouseButton",
            InputEvent::MouseScroll { .. } => "MouseScroll",
            InputEvent::KeyPress { .. } => "KeyPress",
        };
        
        let latency_event = InputLatencyEvent {
            device_id: device_id.to_string(),
            latency_ms,
            event_type: event_type.to_string(),
            timestamp: Instant::now(),
        };
        
        // Send latency event (non-blocking)
        let _ = self.latency_tx.try_send(latency_event);
        
        // Warn if latency exceeds threshold (50ms as per requirements)
        if latency_ms > 50 {
            log::warn!("Input latency exceeded 50ms: {}ms for {} to device {}", 
                latency_ms, event_type, device_id);
        }
        
        Ok(())
    }
    
    /// Route input to the local device
    async fn route_to_local(&self, event: InputEvent) -> Result<()> {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.local_injector.inject_mouse_move(x, y).await?;
            }
            InputEvent::MouseButton { button, pressed } => {
                self.local_injector.inject_mouse_button(button, pressed).await?;
            }
            InputEvent::MouseScroll { delta_x, delta_y } => {
                self.local_injector.inject_mouse_scroll(delta_x, delta_y).await?;
            }
            InputEvent::KeyPress { key_code, modifiers, pressed } => {
                self.local_injector.inject_key_press(key_code, modifiers, pressed).await?;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl InputRouter for DefaultInputRouter {
    async fn route_input(&self, event: InputEvent) -> Result<String> {
        let active = self.active_device.read().await;
        let target_device = active.as_ref()
            .ok_or_else(|| KvmError::InvalidState("No active device set for input routing".to_string()))?
            .clone();
        drop(active);
        
        // Route to local or remote device
        if target_device == self.local_device_id {
            self.route_to_local(event).await?;
        } else {
            self.route_to_remote(&target_device, event).await?;
        }
        
        Ok(target_device)
    }
    
    fn get_routing_target(&self) -> Option<String> {
        self.active_device.try_read().ok()?.clone()
    }
    
    async fn set_routing_target(&self, device_id: Option<String>) -> Result<()> {
        let mut active = self.active_device.write().await;
        *active = device_id;
        Ok(())
    }
}
