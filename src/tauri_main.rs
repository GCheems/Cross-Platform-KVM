#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use cross_platform_kvm::{
    config::{FileConfigManager, ConfigurationManager},
    discovery::MockDeviceDiscovery,
    ui::UiState,
};
use std::sync::Arc;
use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent, Manager, SystemTrayMenuItem};

fn main() {
    // Initialize logger
    env_logger::init();

    // Create initial system tray menu
    let tray_menu = build_tray_menu(None, &[]);

    let system_tray = SystemTray::new().with_menu(tray_menu);

    // Create discovery service (using mock for now, will be replaced with real implementation)
    let discovery = Arc::new(MockDeviceDiscovery::new());
    let ui_state = Arc::new(UiState::new(discovery));

    // Create config manager
    let config_manager = Arc::new(
        FileConfigManager::with_default_path()
            .expect("Failed to create config manager")
    );

    // Clone for async initialization
    let ui_state_init = ui_state.clone();
    let config_manager_init = config_manager.clone();

    tauri::Builder::default()
        .manage(ui_state)
        .manage(config_manager)
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => {
                if id.starts_with("switch_") {
                    // Handle device switch
                    let device_id = id.strip_prefix("switch_").unwrap().to_string();
                    let ui_state = app.state::<Arc<UiState>>();
                    let ui_state_clone = ui_state.inner().clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = ui_state_clone.switch_to_device(device_id).await {
                            eprintln!("Failed to switch device: {}", e);
                        }
                    });
                } else {
                    match id.as_str() {
                        "show" => {
                            if let Some(window) = app.get_window("main") {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            }
                        }
                        "hide" => {
                            if let Some(window) = app.get_window("main") {
                                window.hide().unwrap();
                            }
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            },
            _ => {}
        })
        .setup(move |app| {
            // Initialize UI state
            let handle = app.handle();
            let ui_state_init_clone = ui_state_init.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = ui_state_init_clone.initialize().await {
                    eprintln!("Failed to initialize UI state: {}", e);
                }
                
                // Load configuration
                if let Err(e) = config_manager_init.load().await {
                    eprintln!("Failed to load configuration: {}", e);
                }
            });
            
            // Check for updates on startup (non-blocking)
            #[cfg(feature = "updater")]
            {
                let app_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;
                    // Wait a bit before checking to not slow down startup
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    
                    match app_handle.updater().check().await {
                        Ok(update_response) => {
                            if update_response.is_update_available() {
                                log::info!("Update available: {}", update_response.latest_version());
                                // Optionally show a notification or dialog
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to check for updates: {}", e);
                        }
                    }
                });
            }
            
            // Start tray update listener
            let app_handle = handle.clone();
            let ui_state_tray = ui_state_init.clone();
            tauri::async_runtime::spawn(async move {
                let mut tray_rx = ui_state_tray.subscribe_tray_updates();
                while let Some(_) = tray_rx.recv().await {
                    // Update tray menu
                    if let Err(e) = update_tray_menu(&app_handle, &ui_state_tray).await {
                        eprintln!("Failed to update tray menu: {}", e);
                    }
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cross_platform_kvm::tauri_commands::get_devices,
            cross_platform_kvm::tauri_commands::refresh_devices,
            cross_platform_kvm::tauri_commands::connect_device,
            cross_platform_kvm::tauri_commands::disconnect_device,
            cross_platform_kvm::tauri_commands::listen_device_events,
            cross_platform_kvm::tauri_commands::get_layout_config,
            cross_platform_kvm::tauri_commands::save_layout_config,
            cross_platform_kvm::tauri_commands::update_device_position,
            cross_platform_kvm::tauri_commands::update_edge_switching,
            cross_platform_kvm::tauri_commands::get_hotkeys,
            cross_platform_kvm::tauri_commands::save_hotkey,
            cross_platform_kvm::tauri_commands::remove_hotkey,
            cross_platform_kvm::tauri_commands::check_hotkey_conflict,
            cross_platform_kvm::tauri_commands::get_active_device,
            cross_platform_kvm::tauri_commands::switch_to_device,
            cross_platform_kvm::tauri_commands::get_tray_state,
            cross_platform_kvm::tauri_commands::get_platform,
            cross_platform_kvm::tauri_commands::is_onboarding_completed,
            cross_platform_kvm::tauri_commands::complete_onboarding,
            cross_platform_kvm::tauri_commands::check_permissions,
            cross_platform_kvm::tauri_commands::has_all_required_permissions,
            cross_platform_kvm::tauri_commands::get_platform_setup_instructions,
            cross_platform_kvm::tauri_commands::request_admin_permission,
            cross_platform_kvm::tauri_commands::configure_firewall_windows,
            cross_platform_kvm::tauri_commands::get_firewall_instructions,
            cross_platform_kvm::tauri_commands::open_accessibility_preferences,
            cross_platform_kvm::tauri_commands::open_input_monitoring_preferences,
            cross_platform_kvm::tauri_commands::get_local_device_info,
            cross_platform_kvm::tauri_commands::check_for_updates,
            cross_platform_kvm::tauri_commands::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the system tray menu with current state
fn build_tray_menu(active_device: Option<&str>, connected_devices: &[(String, String)]) -> SystemTrayMenu {
    let mut menu = SystemTrayMenu::new();
    
    // Active device indicator
    if let Some(device_name) = active_device {
        menu = menu.add_item(
            CustomMenuItem::new("active".to_string(), format!("Active: {}", device_name))
                .disabled()
        );
        menu = menu.add_native_item(SystemTrayMenuItem::Separator);
    } else {
        menu = menu.add_item(
            CustomMenuItem::new("active".to_string(), "Active: Local Device")
                .disabled()
        );
        menu = menu.add_native_item(SystemTrayMenuItem::Separator);
    }
    
    // Quick switch menu
    if !connected_devices.is_empty() {
        for (device_id, device_name) in connected_devices {
            let is_active = active_device == Some(device_name);
            let label = if is_active {
                format!("✓ {}", device_name)
            } else {
                device_name.clone()
            };
            
            let item = CustomMenuItem::new(format!("switch_{}", device_id), label);
            let item = if is_active { item.disabled() } else { item };
            menu = menu.add_item(item);
        }
        menu = menu.add_native_item(SystemTrayMenuItem::Separator);
    }
    
    // Standard menu items
    menu = menu
        .add_item(CustomMenuItem::new("show".to_string(), "Show Window"))
        .add_item(CustomMenuItem::new("hide".to_string(), "Hide Window"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit".to_string(), "Quit"));
    
    menu
}

/// Update the tray menu with current state
async fn update_tray_menu(app: &tauri::AppHandle, ui_state: &Arc<UiState>) -> Result<(), Box<dyn std::error::Error>> {
    let tray_state = ui_state.get_tray_state().await;
    
    let active_device_name = tray_state.active_device.as_deref();
    let connected_devices: Vec<(String, String)> = tray_state.connected_devices
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    
    let new_menu = build_tray_menu(active_device_name, &connected_devices);
    
    app.tray_handle().set_menu(new_menu)?;
    
    Ok(())
}
