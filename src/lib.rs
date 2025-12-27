// Cross-platform KVM System Library
// Core modules and trait definitions

pub mod device;
pub mod network;
pub mod input;
pub mod clipboard;
pub mod config;
pub mod switch;
pub mod error;
pub mod protocol;
pub mod discovery;
pub mod security;
pub mod hotkey;
pub mod state;
pub mod recovery;
pub mod ui;
pub mod tauri_commands;
pub mod permissions;
pub mod batching;

// Re-export commonly used types
pub use error::{Result, KvmError};
