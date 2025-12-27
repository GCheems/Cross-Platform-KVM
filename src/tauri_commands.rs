use crate::config::{ConfigurationManager, DeviceLayout, DevicePosition, EdgeSwitchingConfig, FileConfigManager, Hotkey};
use crate::ui::{DeviceInfo, UiEvent, UiState, TrayState};
use crate::input::Modifiers;
use crate::permissions::{
    check_all_permissions, get_setup_instructions, has_all_permissions,
    PermissionStatus as PermStatus,
};
use tauri::State;
use std::sync::Arc;
use std::collections::HashMap;

/// Get all discovered devices
#[tauri::command]
pub async fn get_devices(state: State<'_, Arc<UiState>>) -> std::result::Result<Vec<DeviceInfo>, String> {
    Ok(state.get_devices().await)
}

/// Refresh the device list
#[tauri::command]
pub async fn refresh_devices(state: State<'_, Arc<UiState>>) -> std::result::Result<Vec<DeviceInfo>, String> {
    state.refresh_devices().await
        .map_err(|e| e.to_string())
}

/// Connect to a device
#[tauri::command]
pub async fn connect_device(
    device_id: String,
    state: State<'_, Arc<UiState>>,
) -> std::result::Result<(), String> {
    state.connect_device(device_id).await
        .map_err(|e| e.to_string())
}

/// Disconnect from a device
#[tauri::command]
pub async fn disconnect_device(
    device_id: String,
    state: State<'_, Arc<UiState>>,
) -> std::result::Result<(), String> {
    state.disconnect_device(device_id).await
        .map_err(|e| e.to_string())
}

/// Listen for device events (called from frontend to establish event stream)
#[tauri::command]
pub async fn listen_device_events(
    window: tauri::Window,
    state: State<'_, Arc<UiState>>,
) -> std::result::Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<UiEvent>(100);
    
    // Set the event channel in state
    state.set_event_channel(tx).await;
    
    // Spawn task to forward events to the window
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = window.emit("device-event", event);
        }
    });
    
    Ok(())
}

/// Get layout configuration
#[tauri::command]
pub async fn get_layout_config(
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<DeviceLayout, String> {
    let config = config_manager.get_config().await;
    Ok(config.device_layout)
}

/// Save layout configuration
#[tauri::command]
pub async fn save_layout_config(
    layout: DeviceLayout,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    let mut config = config_manager.get_config().await;
    config.device_layout = layout;
    config_manager.save(&config).await
        .map_err(|e| e.to_string())
}

/// Update device position in layout
#[tauri::command]
pub async fn update_device_position(
    device_id: String,
    position: DevicePosition,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    config_manager.update_device_position(device_id, position).await
        .map_err(|e| e.to_string())
}

/// Update edge switching configuration for a device
#[tauri::command]
pub async fn update_edge_switching(
    device_id: String,
    edge_config: EdgeSwitchingConfig,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    config_manager.update_edge_switching(device_id, edge_config).await
        .map_err(|e| e.to_string())
}

/// Get all hotkeys
#[tauri::command]
pub async fn get_hotkeys(
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<HashMap<String, Hotkey>, String> {
    let config = config_manager.get_config().await;
    Ok(config.hotkeys)
}

/// Add or update a hotkey
#[tauri::command]
pub async fn save_hotkey(
    hotkey_id: String,
    hotkey: Hotkey,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    config_manager.update_hotkey(hotkey_id, hotkey).await
        .map_err(|e| e.to_string())
}

/// Remove a hotkey
#[tauri::command]
pub async fn remove_hotkey(
    hotkey_id: String,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    config_manager.remove_hotkey(&hotkey_id).await
        .map_err(|e| e.to_string())
}

/// Check if a hotkey combination conflicts with existing hotkeys
#[tauri::command]
pub async fn check_hotkey_conflict(
    key_code: u32,
    modifiers: Modifiers,
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<Option<ConflictInfo>, String> {
    let config = config_manager.get_config().await;
    
    // Check for conflicts with existing hotkeys
    for (existing_id, existing_hotkey) in &config.hotkeys {
        if existing_hotkey.key_code == key_code 
            && existing_hotkey.modifiers == modifiers {
            return Ok(Some(ConflictInfo {
                hotkey_id: existing_id.clone(),
                device_id: existing_hotkey.device_id.clone(),
            }));
        }
    }
    
    Ok(None)
}

/// Conflict information for hotkey validation
#[derive(serde::Serialize)]
pub struct ConflictInfo {
    pub hotkey_id: String,
    pub device_id: String,
}

/// Get the currently active device
#[tauri::command]
pub async fn get_active_device(state: State<'_, Arc<UiState>>) -> std::result::Result<Option<DeviceInfo>, String> {
    Ok(state.get_active_device().await)
}

/// Switch to a specific device
#[tauri::command]
pub async fn switch_to_device(
    device_id: String,
    state: State<'_, Arc<UiState>>,
) -> std::result::Result<(), String> {
    state.switch_to_device(device_id).await
        .map_err(|e| e.to_string())
}

/// Get tray state information
#[tauri::command]
pub async fn get_tray_state(state: State<'_, Arc<UiState>>) -> std::result::Result<TrayState, String> {
    Ok(state.get_tray_state().await)
}

/// Get the current platform
#[tauri::command]
pub fn get_platform() -> String {
    #[cfg(target_os = "windows")]
    return "Windows".to_string();
    
    #[cfg(target_os = "macos")]
    return "MacOS".to_string();
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return "Unknown".to_string();
}

/// Check if onboarding has been completed
#[tauri::command]
pub async fn is_onboarding_completed(
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<bool, String> {
    let config = config_manager.get_config().await;
    Ok(config.preferences.onboarding_completed)
}

/// Mark onboarding as completed
#[tauri::command]
pub async fn complete_onboarding(
    config_manager: State<'_, Arc<FileConfigManager>>,
) -> std::result::Result<(), String> {
    config_manager.set_onboarding_completed(true).await
        .map_err(|e| e.to_string())
}

/// Check all required permissions
#[tauri::command]
pub async fn check_permissions() -> std::result::Result<PermStatus, String> {
    check_all_permissions()
        .map_err(|e| e.to_string())
}

/// Check if all required permissions are granted
#[tauri::command]
pub async fn has_all_required_permissions() -> std::result::Result<bool, String> {
    has_all_permissions()
        .map_err(|e| e.to_string())
}

/// Get platform-specific setup instructions
#[tauri::command]
pub async fn get_platform_setup_instructions() -> std::result::Result<String, String> {
    Ok(get_setup_instructions())
}

/// Request admin permission (Windows only)
#[tauri::command]
pub async fn request_admin_permission() -> std::result::Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::permissions::request_admin_windows()
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("Not applicable on this platform".to_string())
    }
}

/// Configure firewall automatically (Windows only, requires admin)
#[tauri::command]
pub async fn configure_firewall_windows() -> std::result::Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::permissions::configure_firewall_windows()
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Err("Not applicable on this platform".to_string())
    }
}

/// Get firewall configuration instructions
#[tauri::command]
pub async fn get_firewall_instructions() -> std::result::Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(crate::permissions::get_firewall_instructions_windows())
    }
    
    #[cfg(target_os = "macos")]
    {
        Ok(crate::permissions::get_firewall_instructions_macos())
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Not applicable on this platform".to_string())
    }
}

/// Open accessibility preferences (macOS only)
#[tauri::command]
pub async fn open_accessibility_preferences() -> std::result::Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::permissions::request_accessibility_macos()
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not applicable on this platform".to_string())
    }
}

/// Open input monitoring preferences (macOS only)
#[tauri::command]
pub async fn open_input_monitoring_preferences() -> std::result::Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::permissions::request_input_monitoring_macos()
            .map_err(|e| e.to_string())
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not applicable on this platform".to_string())
    }
}

/// Get local device information
#[tauri::command]
pub async fn get_local_device_info(state: State<'_, Arc<UiState>>) -> std::result::Result<DeviceInfo, String> {
    state.get_local_device_info().await
        .map_err(|e| e.to_string())
}

/// Check for application updates
#[tauri::command]
pub async fn check_for_updates(_app_handle: tauri::AppHandle) -> std::result::Result<UpdateInfo, String> {
    #[cfg(feature = "updater")]
    {
        use tauri::updater::UpdateResponse;
        
        match _app_handle.updater().check().await {
            Ok(update_response) => {
                if update_response.is_update_available() {
                    Ok(UpdateInfo {
                        available: true,
                        version: update_response.latest_version().to_string(),
                        current_version: _app_handle.package_info().version.to_string(),
                        release_notes: update_response.body().map(|s| s.to_string()),
                        release_date: update_response.date().map(|s| s.to_string()),
                    })
                } else {
                    Ok(UpdateInfo {
                        available: false,
                        version: _app_handle.package_info().version.to_string(),
                        current_version: _app_handle.package_info().version.to_string(),
                        release_notes: None,
                        release_date: None,
                    })
                }
            }
            Err(e) => Err(format!("Failed to check for updates: {}", e)),
        }
    }
    
    #[cfg(not(feature = "updater"))]
    {
        Err("Updater feature not enabled".to_string())
    }
}

/// Download and install an update
#[tauri::command]
pub async fn install_update(_app_handle: tauri::AppHandle) -> std::result::Result<(), String> {
    #[cfg(feature = "updater")]
    {
        match _app_handle.updater().check().await {
            Ok(update_response) => {
                if update_response.is_update_available() {
                    update_response.download_and_install().await
                        .map_err(|e| format!("Failed to install update: {}", e))?;
                    Ok(())
                } else {
                    Err("No update available".to_string())
                }
            }
            Err(e) => Err(format!("Failed to check for updates: {}", e)),
        }
    }
    
    #[cfg(not(feature = "updater"))]
    {
        Err("Updater feature not enabled".to_string())
    }
}

/// Information about available updates
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub current_version: String,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
}
