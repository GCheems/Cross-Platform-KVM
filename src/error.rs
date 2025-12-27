use thiserror::Error;

/// Result type for KVM operations
pub type Result<T> = std::result::Result<T, KvmError>;

/// Error types for the KVM system
#[derive(Error, Debug)]
pub enum KvmError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Input capture error: {0}")]
    InputCapture(String),

    #[error("Input injection error: {0}")]
    InputInjection(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Permission error: {0}")]
    Permission(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Hotkey conflict: {0}")]
    HotkeyConflict(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
