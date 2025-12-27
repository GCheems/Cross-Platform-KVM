use crate::{Result, KvmError};
use crate::config::Hotkey;
use crate::input::Modifiers;
use crate::switch::SwitchController;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Hotkey event triggered when a registered hotkey is pressed
#[derive(Debug, Clone)]
pub struct HotkeyEvent {
    pub hotkey_id: String,
    pub device_id: String,
    pub key_code: u32,
    pub modifiers: Modifiers,
}

/// Hotkey conflict error details
#[derive(Debug, Clone)]
pub struct HotkeyConflict {
    pub existing_hotkey_id: String,
    pub existing_device_id: String,
    pub key_code: u32,
    pub modifiers: Modifiers,
}

/// Trait for hotkey management
#[async_trait::async_trait]
pub trait HotkeyManager: Send + Sync {
    /// Register a global hotkey
    /// Returns an error if the hotkey conflicts with an existing one
    async fn register_hotkey(&self, hotkey_id: String, hotkey: Hotkey) -> Result<()>;

    /// Unregister a hotkey
    async fn unregister_hotkey(&self, hotkey_id: &str) -> Result<()>;

    /// Enable a hotkey (if it was disabled)
    async fn enable_hotkey(&self, hotkey_id: &str) -> Result<()>;

    /// Disable a hotkey (without unregistering it)
    async fn disable_hotkey(&self, hotkey_id: &str) -> Result<()>;

    /// Check if a hotkey combination conflicts with existing hotkeys
    async fn check_conflict(&self, key_code: u32, modifiers: Modifiers) -> Option<HotkeyConflict>;

    /// Get all registered hotkeys
    async fn get_hotkeys(&self) -> HashMap<String, Hotkey>;

    /// Subscribe to hotkey events
    fn subscribe(&self) -> mpsc::Receiver<HotkeyEvent>;

    /// Start listening for hotkey presses
    async fn start_listening(&self) -> Result<()>;

    /// Stop listening for hotkey presses
    async fn stop_listening(&self) -> Result<()>;
}

/// Key combination for hotkey matching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct KeyCombination {
    key_code: u32,
    shift: bool,
    ctrl: bool,
    alt: bool,
    meta: bool,
}

impl KeyCombination {
    fn from_modifiers(key_code: u32, modifiers: Modifiers) -> Self {
        Self {
            key_code,
            shift: modifiers.shift,
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            meta: modifiers.meta,
        }
    }

    fn to_modifiers(&self) -> Modifiers {
        Modifiers {
            shift: self.shift,
            ctrl: self.ctrl,
            alt: self.alt,
            meta: self.meta,
        }
    }
}

/// Default implementation of hotkey manager
pub struct DefaultHotkeyManager {
    /// Registered hotkeys by ID
    hotkeys: Arc<RwLock<HashMap<String, Hotkey>>>,

    /// Reverse mapping: key combination -> hotkey ID
    key_map: Arc<RwLock<HashMap<KeyCombination, String>>>,

    /// Switch controller for triggering device switches
    switch_controller: Arc<dyn SwitchController>,

    /// Event broadcaster
    event_tx: mpsc::Sender<HotkeyEvent>,

    /// Event receiver (for subscription)
    event_rx: Arc<RwLock<Option<mpsc::Receiver<HotkeyEvent>>>>,

    /// Whether the manager is currently listening
    listening: Arc<RwLock<bool>>,

    /// System hotkeys that should not be overridden (platform-specific)
    system_hotkeys: Arc<RwLock<Vec<KeyCombination>>>,
}

impl DefaultHotkeyManager {
    /// Create a new hotkey manager
    pub fn new(switch_controller: Arc<dyn SwitchController>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);

        Self {
            hotkeys: Arc::new(RwLock::new(HashMap::new())),
            key_map: Arc::new(RwLock::new(HashMap::new())),
            switch_controller,
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
            listening: Arc::new(RwLock::new(false)),
            system_hotkeys: Arc::new(RwLock::new(Self::get_system_hotkeys())),
        }
    }

    /// Get platform-specific system hotkeys that should not be overridden
    fn get_system_hotkeys() -> Vec<KeyCombination> {
        let mut system_keys = Vec::new();

        #[cfg(target_os = "windows")]
        {
            // Windows system hotkeys
            // Win+L (lock screen)
            system_keys.push(KeyCombination {
                key_code: 0x4C, // L key
                shift: false,
                ctrl: false,
                alt: false,
                meta: true,
            });

            // Ctrl+Alt+Del
            system_keys.push(KeyCombination {
                key_code: 0x2E, // Delete key
                shift: false,
                ctrl: true,
                alt: true,
                meta: false,
            });

            // Alt+Tab
            system_keys.push(KeyCombination {
                key_code: 0x09, // Tab key
                shift: false,
                ctrl: false,
                alt: true,
                meta: false,
            });

            // Alt+F4
            system_keys.push(KeyCombination {
                key_code: 0x73, // F4 key
                shift: false,
                ctrl: false,
                alt: true,
                meta: false,
            });
        }

        #[cfg(target_os = "macos")]
        {
            // macOS system hotkeys
            // Cmd+Q (quit)
            system_keys.push(KeyCombination {
                key_code: 0x0C, // Q key
                shift: false,
                ctrl: false,
                alt: false,
                meta: true,
            });

            // Cmd+Tab
            system_keys.push(KeyCombination {
                key_code: 0x30, // Tab key
                shift: false,
                ctrl: false,
                alt: false,
                meta: true,
            });

            // Cmd+Space (Spotlight)
            system_keys.push(KeyCombination {
                key_code: 0x31, // Space key
                shift: false,
                ctrl: false,
                alt: false,
                meta: true,
            });

            // Ctrl+Cmd+Q (lock screen)
            system_keys.push(KeyCombination {
                key_code: 0x0C, // Q key
                shift: false,
                ctrl: true,
                alt: false,
                meta: true,
            });
        }

        system_keys
    }

    /// Check if a key combination is a system hotkey
    async fn is_system_hotkey(&self, key_combo: &KeyCombination) -> bool {
        let system_hotkeys = self.system_hotkeys.read().await;
        system_hotkeys.contains(key_combo)
    }

    /// Handle a key press event (called by the input capture system)
    pub async fn handle_key_press(&self, key_code: u32, modifiers: Modifiers, pressed: bool) -> Result<()> {
        // Only handle key down events
        if !pressed {
            return Ok(());
        }

        // Check if we're listening
        let listening = self.listening.read().await;
        if !*listening {
            return Ok(());
        }
        drop(listening);

        // Create key combination
        let key_combo = KeyCombination::from_modifiers(key_code, modifiers);

        // Look up the hotkey
        let key_map = self.key_map.read().await;
        let hotkey_id = match key_map.get(&key_combo) {
            Some(id) => id.clone(),
            None => return Ok(()), // Not a registered hotkey
        };
        drop(key_map);

        // Get the hotkey details
        let hotkeys = self.hotkeys.read().await;
        let hotkey = match hotkeys.get(&hotkey_id) {
            Some(h) => h.clone(),
            None => return Ok(()), // Hotkey was unregistered
        };
        drop(hotkeys);

        // Check if hotkey is enabled
        if !hotkey.enabled {
            return Ok(());
        }

        // Trigger device switch
        let device_id = hotkey.device_id.clone();
        match self.switch_controller.switch_to(&device_id).await {
            Ok(()) => {
                // Send hotkey event
                let event = HotkeyEvent {
                    hotkey_id: hotkey_id.clone(),
                    device_id: device_id.clone(),
                    key_code,
                    modifiers,
                };
                let _ = self.event_tx.send(event).await;
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to switch to device {} via hotkey {}: {}", device_id, hotkey_id, e);
                Err(e)
            }
        }
    }

    /// Validate that a hotkey doesn't conflict with system hotkeys or existing hotkeys
    async fn validate_hotkey(&self, key_code: u32, modifiers: Modifiers, exclude_id: Option<&str>) -> Result<()> {
        let key_combo = KeyCombination::from_modifiers(key_code, modifiers);

        // Check for system hotkey conflict
        if self.is_system_hotkey(&key_combo).await {
            return Err(KvmError::HotkeyConflict(format!(
                "Hotkey conflicts with system hotkey: key_code={}, modifiers={:?}",
                key_code, modifiers
            )));
        }

        // Check for existing hotkey conflict
        let key_map = self.key_map.read().await;
        if let Some(existing_id) = key_map.get(&key_combo) {
            // If we're updating an existing hotkey, allow the same ID
            if let Some(exclude) = exclude_id {
                if existing_id == exclude {
                    return Ok(());
                }
            }

            let hotkeys = self.hotkeys.read().await;
            if let Some(existing_hotkey) = hotkeys.get(existing_id) {
                return Err(KvmError::HotkeyConflict(format!(
                    "Hotkey conflicts with existing hotkey '{}' for device '{}'",
                    existing_id, existing_hotkey.device_id
                )));
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl HotkeyManager for DefaultHotkeyManager {
    async fn register_hotkey(&self, hotkey_id: String, hotkey: Hotkey) -> Result<()> {
        // Validate the hotkey
        self.validate_hotkey(hotkey.key_code, hotkey.modifiers, Some(&hotkey_id)).await?;

        // Validate device_id is not empty
        if hotkey.device_id.is_empty() {
            return Err(KvmError::InvalidInput("Hotkey device_id cannot be empty".to_string()));
        }

        let key_combo = KeyCombination::from_modifiers(hotkey.key_code, hotkey.modifiers);

        // Register the hotkey
        let mut hotkeys = self.hotkeys.write().await;
        let mut key_map = self.key_map.write().await;

        // Remove old key combination if this hotkey ID was already registered
        if let Some(old_hotkey) = hotkeys.get(&hotkey_id) {
            let old_combo = KeyCombination::from_modifiers(old_hotkey.key_code, old_hotkey.modifiers);
            key_map.remove(&old_combo);
        }

        // Add new hotkey
        hotkeys.insert(hotkey_id.clone(), hotkey);
        key_map.insert(key_combo, hotkey_id);

        Ok(())
    }

    async fn unregister_hotkey(&self, hotkey_id: &str) -> Result<()> {
        let mut hotkeys = self.hotkeys.write().await;
        let mut key_map = self.key_map.write().await;

        // Remove from hotkeys map
        if let Some(hotkey) = hotkeys.remove(hotkey_id) {
            // Remove from key map
            let key_combo = KeyCombination::from_modifiers(hotkey.key_code, hotkey.modifiers);
            key_map.remove(&key_combo);
        }

        Ok(())
    }

    async fn enable_hotkey(&self, hotkey_id: &str) -> Result<()> {
        let mut hotkeys = self.hotkeys.write().await;

        let hotkey = hotkeys.get_mut(hotkey_id)
            .ok_or_else(|| KvmError::NotFound(format!("Hotkey '{}' not found", hotkey_id)))?;

        hotkey.enabled = true;
        Ok(())
    }

    async fn disable_hotkey(&self, hotkey_id: &str) -> Result<()> {
        let mut hotkeys = self.hotkeys.write().await;

        let hotkey = hotkeys.get_mut(hotkey_id)
            .ok_or_else(|| KvmError::NotFound(format!("Hotkey '{}' not found", hotkey_id)))?;

        hotkey.enabled = false;
        Ok(())
    }

    async fn check_conflict(&self, key_code: u32, modifiers: Modifiers) -> Option<HotkeyConflict> {
        let key_combo = KeyCombination::from_modifiers(key_code, modifiers);

        // Check system hotkeys
        if self.is_system_hotkey(&key_combo).await {
            return Some(HotkeyConflict {
                existing_hotkey_id: "system".to_string(),
                existing_device_id: "system".to_string(),
                key_code,
                modifiers,
            });
        }

        // Check existing hotkeys
        let key_map = self.key_map.read().await;
        if let Some(existing_id) = key_map.get(&key_combo) {
            let hotkeys = self.hotkeys.read().await;
            if let Some(existing_hotkey) = hotkeys.get(existing_id) {
                return Some(HotkeyConflict {
                    existing_hotkey_id: existing_id.clone(),
                    existing_device_id: existing_hotkey.device_id.clone(),
                    key_code,
                    modifiers,
                });
            }
        }

        None
    }

    async fn get_hotkeys(&self) -> HashMap<String, Hotkey> {
        self.hotkeys.read().await.clone()
    }

    fn subscribe(&self) -> mpsc::Receiver<HotkeyEvent> {
        // Take the receiver if available, otherwise create a new channel
        let mut rx_lock = futures::executor::block_on(self.event_rx.write());
        if let Some(rx) = rx_lock.take() {
            rx
        } else {
            // Create a new receiver (this shouldn't happen in normal usage)
            let (_, rx) = mpsc::channel(100);
            rx
        }
    }

    async fn start_listening(&self) -> Result<()> {
        let mut listening = self.listening.write().await;
        *listening = true;
        Ok(())
    }

    async fn stop_listening(&self) -> Result<()> {
        let mut listening = self.listening.write().await;
        *listening = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::switch::{DefaultSwitchController, SwitchEvent};
    use crate::device::Device;

    #[tokio::test]
    async fn test_register_hotkey() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);

        let hotkey = Hotkey {
            key_code: 65, // 'A' key
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                alt: false,
                meta: false,
            },
            device_id: "device1".to_string(),
            enabled: true,
        };

        let result = manager.register_hotkey("hotkey1".to_string(), hotkey).await;
        assert!(result.is_ok());

        let hotkeys = manager.get_hotkeys().await;
        assert_eq!(hotkeys.len(), 1);
        assert!(hotkeys.contains_key("hotkey1"));
    }

    #[tokio::test]
    async fn test_hotkey_conflict_detection() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);

        let hotkey1 = Hotkey {
            key_code: 65,
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                alt: false,
                meta: false,
            },
            device_id: "device1".to_string(),
            enabled: true,
        };

        // Register first hotkey
        manager.register_hotkey("hotkey1".to_string(), hotkey1.clone()).await.unwrap();

        // Try to register conflicting hotkey
        let hotkey2 = Hotkey {
            key_code: 65,
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                alt: false,
                meta: false,
            },
            device_id: "device2".to_string(),
            enabled: true,
        };

        let result = manager.register_hotkey("hotkey2".to_string(), hotkey2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_enable_disable_hotkey() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);

        let hotkey = Hotkey {
            key_code: 65,
            modifiers: Modifiers::default(),
            device_id: "device1".to_string(),
            enabled: true,
        };

        manager.register_hotkey("hotkey1".to_string(), hotkey).await.unwrap();

        // Disable hotkey
        manager.disable_hotkey("hotkey1").await.unwrap();
        let hotkeys = manager.get_hotkeys().await;
        assert!(!hotkeys.get("hotkey1").unwrap().enabled);

        // Enable hotkey
        manager.enable_hotkey("hotkey1").await.unwrap();
        let hotkeys = manager.get_hotkeys().await;
        assert!(hotkeys.get("hotkey1").unwrap().enabled);
    }

    #[tokio::test]
    async fn test_unregister_hotkey() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);

        let hotkey = Hotkey {
            key_code: 65,
            modifiers: Modifiers::default(),
            device_id: "device1".to_string(),
            enabled: true,
        };

        manager.register_hotkey("hotkey1".to_string(), hotkey).await.unwrap();
        assert_eq!(manager.get_hotkeys().await.len(), 1);

        manager.unregister_hotkey("hotkey1").await.unwrap();
        assert_eq!(manager.get_hotkeys().await.len(), 0);
    }

    #[tokio::test]
    async fn test_hotkey_uniqueness() {
        let switch_controller = Arc::new(DefaultSwitchController::new(200));
        let manager = DefaultHotkeyManager::new(switch_controller);

        // Register hotkeys for different devices
        let hotkey1 = Hotkey {
            key_code: 65,
            modifiers: Modifiers { shift: true, ctrl: false, alt: false, meta: false },
            device_id: "device1".to_string(),
            enabled: true,
        };

        let hotkey2 = Hotkey {
            key_code: 66,
            modifiers: Modifiers { shift: true, ctrl: false, alt: false, meta: false },
            device_id: "device2".to_string(),
            enabled: true,
        };

        manager.register_hotkey("hotkey1".to_string(), hotkey1).await.unwrap();
        manager.register_hotkey("hotkey2".to_string(), hotkey2).await.unwrap();

        // Verify both are registered
        let hotkeys = manager.get_hotkeys().await;
        assert_eq!(hotkeys.len(), 2);

        // Verify they have different key combinations
        let h1 = hotkeys.get("hotkey1").unwrap();
        let h2 = hotkeys.get("hotkey2").unwrap();
        assert_ne!(h1.key_code, h2.key_code);
    }
}
