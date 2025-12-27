use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

/// Device position in the layout
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevicePosition {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Edge switching configuration for each edge
    #[serde(default)]
    pub edge_switching: EdgeSwitchingConfig,
}

impl DevicePosition {
    /// Create a new DevicePosition with default edge switching enabled
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            edge_switching: EdgeSwitchingConfig::default(),
        }
    }
}

/// Edge switching configuration for a device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeSwitchingConfig {
    pub left_enabled: bool,
    pub right_enabled: bool,
    pub top_enabled: bool,
    pub bottom_enabled: bool,
}

impl Default for EdgeSwitchingConfig {
    fn default() -> Self {
        Self {
            left_enabled: true,
            right_enabled: true,
            top_enabled: true,
            bottom_enabled: true,
        }
    }
}

/// Device layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLayout {
    pub devices: HashMap<String, DevicePosition>,
}

/// Hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkey {
    pub key_code: u32,
    pub modifiers: crate::input::Modifiers,
    pub device_id: String,
    pub enabled: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub authorized_devices: Vec<String>,
    pub require_authorization: bool,
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub edge_switch_delay_ms: u64,
    pub clipboard_sync_enabled: bool,
    pub clipboard_size_limit_mb: u64,
    pub show_notifications: bool,
    pub network_timeout_ms: u64,
    #[serde(default)]
    pub onboarding_completed: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            edge_switch_delay_ms: 200,
            clipboard_sync_enabled: true,
            clipboard_size_limit_mb: 10,
            show_notifications: true,
            network_timeout_ms: 5000,
            onboarding_completed: false,
        }
    }
}

/// Complete application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device_layout: DeviceLayout,
    pub hotkeys: HashMap<String, Hotkey>,
    pub security: SecurityConfig,
    pub preferences: Preferences,
}

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    LayoutChanged,
    HotkeyChanged(String),
    SecurityChanged,
    PreferencesChanged,
}

/// Trait for configuration management
#[async_trait::async_trait]
pub trait ConfigurationManager: Send + Sync {
    /// Load configuration from disk
    async fn load(&self) -> Result<Config>;

    /// Save configuration to disk
    async fn save(&self, config: &Config) -> Result<()>;

    /// Subscribe to configuration change events
    fn subscribe(&self) -> Receiver<ConfigEvent>;
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_layout: DeviceLayout {
                devices: std::collections::HashMap::new(),
            },
            hotkeys: std::collections::HashMap::new(),
            security: SecurityConfig {
                authorized_devices: Vec::new(),
                require_authorization: true,
            },
            preferences: Preferences::default(),
        }
    }
}

impl Default for DeviceLayout {
    fn default() -> Self {
        Self {
            devices: std::collections::HashMap::new(),
        }
    }
}

/// File-based configuration manager implementation
pub struct FileConfigManager {
    config_path: std::path::PathBuf,
    event_tx: tokio::sync::broadcast::Sender<ConfigEvent>,
    current_config: tokio::sync::RwLock<Config>,
}

impl FileConfigManager {
    /// Create a new file-based configuration manager
    pub fn new(config_path: std::path::PathBuf) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            config_path,
            event_tx,
            current_config: tokio::sync::RwLock::new(Config::default()),
        }
    }

    /// Create a configuration manager with default path
    pub fn with_default_path() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| crate::KvmError::ConfigError("Cannot determine config directory".to_string()))?;
        let app_config_dir = config_dir.join("cross-platform-kvm");
        std::fs::create_dir_all(&app_config_dir)
            .map_err(|e| crate::KvmError::ConfigError(format!("Failed to create config directory: {}", e)))?;
        let config_path = app_config_dir.join("config.json");
        Ok(Self::new(config_path))
    }

    /// Validate configuration
    fn validate_config(&self, config: &Config) -> Result<()> {
        // Validate device layout
        for (device_id, position) in &config.device_layout.devices {
            if position.width <= 0 || position.height <= 0 {
                return Err(crate::KvmError::ConfigError(
                    format!("Invalid device dimensions for {}: width and height must be positive", device_id)
                ));
            }
        }

        // Validate hotkeys
        let mut used_keys = std::collections::HashSet::new();
        for (hotkey_id, hotkey) in &config.hotkeys {
            // Check for duplicate hotkey combinations
            let key_combo = (hotkey.key_code, hotkey.modifiers.shift, hotkey.modifiers.ctrl, 
                           hotkey.modifiers.alt, hotkey.modifiers.meta);
            if !used_keys.insert(key_combo) {
                return Err(crate::KvmError::ConfigError(
                    format!("Duplicate hotkey combination for {}", hotkey_id)
                ));
            }

            // Validate device_id is not empty
            if hotkey.device_id.is_empty() {
                return Err(crate::KvmError::ConfigError(
                    format!("Hotkey {} has empty device_id", hotkey_id)
                ));
            }
        }

        // Validate preferences
        if config.preferences.edge_switch_delay_ms > 5000 {
            return Err(crate::KvmError::ConfigError(
                "Edge switch delay cannot exceed 5000ms".to_string()
            ));
        }

        if config.preferences.clipboard_size_limit_mb == 0 {
            return Err(crate::KvmError::ConfigError(
                "Clipboard size limit must be greater than 0".to_string()
            ));
        }

        Ok(())
    }

    /// Update device position in layout
    pub async fn update_device_position(&self, device_id: String, position: DevicePosition) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.device_layout.devices.insert(device_id, position);
        
        // Validate the updated config
        self.validate_config(&config)?;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::LayoutChanged);
        
        Ok(())
    }

    /// Update edge switching configuration for a device
    pub async fn update_edge_switching(&self, device_id: String, edge_config: EdgeSwitchingConfig) -> Result<()> {
        let mut config = self.current_config.write().await;
        
        // Get the device position or create a new one
        let position = config.device_layout.devices.entry(device_id.clone())
            .or_insert_with(|| DevicePosition {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                edge_switching: EdgeSwitchingConfig::default(),
            });
        
        // Update edge switching config
        position.edge_switching = edge_config;
        
        // Validate the updated config
        self.validate_config(&config)?;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::LayoutChanged);
        
        Ok(())
    }

    /// Update hotkey configuration
    pub async fn update_hotkey(&self, hotkey_id: String, hotkey: Hotkey) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.hotkeys.insert(hotkey_id.clone(), hotkey);
        
        // Validate the updated config
        self.validate_config(&config)?;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::HotkeyChanged(hotkey_id));
        
        Ok(())
    }

    /// Remove hotkey
    pub async fn remove_hotkey(&self, hotkey_id: &str) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.hotkeys.remove(hotkey_id);
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::HotkeyChanged(hotkey_id.to_string()));
        
        Ok(())
    }

    /// Update security configuration
    pub async fn update_security(&self, security: SecurityConfig) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.security = security;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::SecurityChanged);
        
        Ok(())
    }

    /// Update preferences
    pub async fn update_preferences(&self, preferences: Preferences) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.preferences = preferences;
        
        // Validate the updated config
        self.validate_config(&config)?;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::PreferencesChanged);
        
        Ok(())
    }

    /// Set onboarding completed status
    pub async fn set_onboarding_completed(&self, completed: bool) -> Result<()> {
        let mut config = self.current_config.write().await;
        config.preferences.onboarding_completed = completed;
        
        // Save to disk
        self.save_to_file(&config).await?;
        
        // Notify listeners
        let _ = self.event_tx.send(ConfigEvent::PreferencesChanged);
        
        Ok(())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Config {
        self.current_config.read().await.clone()
    }

    /// Internal method to save config to file
    async fn save_to_file(&self, config: &Config) -> Result<()> {
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| crate::KvmError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        
        tokio::fs::write(&self.config_path, json).await
            .map_err(|e| crate::KvmError::ConfigError(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }

    /// Internal method to load config from file
    async fn load_from_file(&self) -> Result<Config> {
        if !self.config_path.exists() {
            // Return default config if file doesn't exist
            return Ok(Config::default());
        }

        let json = tokio::fs::read_to_string(&self.config_path).await
            .map_err(|e| crate::KvmError::ConfigError(format!("Failed to read config file: {}", e)))?;
        
        let config: Config = serde_json::from_str(&json)
            .map_err(|e| crate::KvmError::ConfigError(format!("Failed to parse config file: {}", e)))?;
        
        // Validate loaded config
        self.validate_config(&config)?;
        
        Ok(config)
    }
}

#[async_trait::async_trait]
impl ConfigurationManager for FileConfigManager {
    async fn load(&self) -> Result<Config> {
        let config = self.load_from_file().await?;
        *self.current_config.write().await = config.clone();
        Ok(config)
    }

    async fn save(&self, config: &Config) -> Result<()> {
        // Validate before saving
        self.validate_config(config)?;
        
        // Update current config
        *self.current_config.write().await = config.clone();
        
        // Save to file
        self.save_to_file(config).await?;
        
        Ok(())
    }

    fn subscribe(&self) -> Receiver<ConfigEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut broadcast_rx = self.event_tx.subscribe();
        
        tokio::spawn(async move {
            while let Ok(event) = broadcast_rx.recv().await {
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });
        
        rx
    }
}
