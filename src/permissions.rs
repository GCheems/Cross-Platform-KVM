// Platform-specific permission and installation handling
// Implements requirements 10.1, 10.2, 10.3, 10.4

use crate::{Result, KvmError};
use std::process::Command;

/// Permission status for the application
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionStatus {
    /// Whether the application has administrator/root privileges
    pub admin: bool,
    /// Whether accessibility permissions are granted (macOS)
    pub accessibility: bool,
    /// Whether input monitoring permissions are granted (macOS)
    pub input_monitoring: bool,
    /// Whether network access is available
    pub network: bool,
    /// Whether firewall configuration is needed
    pub firewall_configured: bool,
}

/// Firewall configuration status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FirewallStatus {
    /// Whether the firewall is active
    pub active: bool,
    /// Whether the application is allowed through the firewall
    pub app_allowed: bool,
    /// Instructions for manual configuration
    pub manual_instructions: Option<String>,
}

/// Check all required permissions for the current platform
pub fn check_all_permissions() -> Result<PermissionStatus> {
    let mut status = PermissionStatus {
        admin: false,
        accessibility: false,
        input_monitoring: false,
        network: true, // Assume network is available by default
        firewall_configured: false,
    };
    
    #[cfg(target_os = "windows")]
    {
        status.admin = check_admin_windows()?;
        status.firewall_configured = check_firewall_windows()?;
    }
    
    #[cfg(target_os = "macos")]
    {
        status.accessibility = check_accessibility_macos()?;
        status.input_monitoring = check_input_monitoring_macos()?;
        status.firewall_configured = check_firewall_macos()?;
    }
    
    Ok(status)
}

/// Request administrator privileges (Windows)
#[cfg(target_os = "windows")]
pub fn request_admin_windows() -> Result<()> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::core::{PCWSTR, w};
    
    // Check if already elevated
    if check_admin_windows()? {
        return Ok(());
    }
    
    // Get current executable path
    let exe_path = std::env::current_exe()
        .map_err(|e| KvmError::Platform(format!("Failed to get executable path: {}", e)))?;
    
    log::info!("Requesting administrator privileges for: {:?}", exe_path);
    
    // Note: In a real implementation, we would use ShellExecuteW with "runas" verb
    // to restart the application with elevated privileges. For now, we return an error
    // instructing the user to restart as administrator.
    
    Err(KvmError::Permission(
        "Administrator privileges required. Please restart the application as Administrator.".to_string()
    ))
}

/// Check if running with administrator privileges (Windows)
#[cfg(target_os = "windows")]
pub fn check_admin_windows() -> Result<bool> {
    use std::ptr;
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::winnt::{TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use winapi::um::handleapi::CloseHandle;
    
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(KvmError::Platform("Failed to open process token".to_string()));
        }
        
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0;
        
        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        
        CloseHandle(token);
        
        if result == 0 {
            return Err(KvmError::Platform("Failed to get token information".to_string()));
        }
        
        Ok(elevation.TokenIsElevated != 0)
    }
}

/// Check firewall configuration (Windows)
#[cfg(target_os = "windows")]
pub fn check_firewall_windows() -> Result<bool> {
    // Check if Windows Firewall allows the application
    // This is a simplified check - in production, you would query the firewall rules
    
    let exe_path = std::env::current_exe()
        .map_err(|e| KvmError::Platform(format!("Failed to get executable path: {}", e)))?;
    
    // Try to check firewall rules using netsh
    let output = Command::new("netsh")
        .args(&["advfirewall", "firewall", "show", "rule", "name=all"])
        .output();
    
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Check if our executable is mentioned in the firewall rules
            Ok(stdout.contains(&exe_path.to_string_lossy().to_string()))
        }
        Err(_) => {
            // If we can't check, assume it's not configured
            Ok(false)
        }
    }
}

/// Get firewall configuration instructions (Windows)
#[cfg(target_os = "windows")]
pub fn get_firewall_instructions_windows() -> String {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "kvm-ui.exe".to_string());
    
    format!(
        "To allow KVM through Windows Firewall:\n\n\
        1. Open Windows Defender Firewall with Advanced Security\n\
        2. Click 'Inbound Rules' in the left panel\n\
        3. Click 'New Rule...' in the right panel\n\
        4. Select 'Program' and click Next\n\
        5. Browse to: {}\n\
        6. Select 'Allow the connection' and click Next\n\
        7. Check all profiles (Domain, Private, Public) and click Next\n\
        8. Name the rule 'KVM System' and click Finish\n\n\
        Alternatively, run this command as Administrator:\n\
        netsh advfirewall firewall add rule name=\"KVM System\" dir=in action=allow program=\"{}\" enable=yes",
        exe_path, exe_path
    )
}

/// Configure firewall automatically (Windows, requires admin)
#[cfg(target_os = "windows")]
pub fn configure_firewall_windows() -> Result<()> {
    if !check_admin_windows()? {
        return Err(KvmError::Permission(
            "Administrator privileges required to configure firewall".to_string()
        ));
    }
    
    let exe_path = std::env::current_exe()
        .map_err(|e| KvmError::Platform(format!("Failed to get executable path: {}", e)))?;
    
    // Add inbound rule
    let output = Command::new("netsh")
        .args(&[
            "advfirewall", "firewall", "add", "rule",
            "name=KVM System",
            "dir=in",
            "action=allow",
            &format!("program={}", exe_path.to_string_lossy()),
            "enable=yes",
        ])
        .output()
        .map_err(|e| KvmError::Platform(format!("Failed to execute netsh: {}", e)))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KvmError::Platform(format!("Failed to add firewall rule: {}", stderr)));
    }
    
    log::info!("Successfully added firewall rule for KVM System");
    Ok(())
}

/// Check accessibility permissions (macOS)
#[cfg(target_os = "macos")]
pub fn check_accessibility_macos() -> Result<bool> {
    // Use core-foundation to check AXIsProcessTrusted
    // This requires linking against ApplicationServices framework
    
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    
    unsafe {
        let is_trusted = AXIsProcessTrusted();
        log::info!("Accessibility permission check: {}", is_trusted);
        Ok(is_trusted)
    }
}

/// Request accessibility permissions (macOS)
#[cfg(target_os = "macos")]
pub fn request_accessibility_macos() -> Result<()> {
    // Open System Preferences to the Accessibility pane
    let output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .output()
        .map_err(|e| KvmError::Platform(format!("Failed to open System Preferences: {}", e)))?;
    
    if !output.status.success() {
        return Err(KvmError::Platform("Failed to open Accessibility preferences".to_string()));
    }
    
    log::info!("Opened Accessibility preferences");
    Ok(())
}

/// Check input monitoring permissions (macOS)
#[cfg(target_os = "macos")]
pub fn check_input_monitoring_macos() -> Result<bool> {
    // Check if input monitoring is enabled by attempting to create an event tap
    // If we can create it, we have permission
    
    use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType};
    
    // Try to create a passive event tap (listen-only)
    let tap_result = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown],
        |_proxy, _event_type, event| {
            Some(event.to_owned())
        },
    );
    
    match tap_result {
        Ok(_tap) => {
            log::info!("Input monitoring permission check: granted");
            Ok(true)
        }
        Err(_) => {
            log::info!("Input monitoring permission check: denied");
            Ok(false)
        }
    }
}

/// Request input monitoring permissions (macOS)
#[cfg(target_os = "macos")]
pub fn request_input_monitoring_macos() -> Result<()> {
    // Open System Preferences to the Input Monitoring pane
    let output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .output()
        .map_err(|e| KvmError::Platform(format!("Failed to open System Preferences: {}", e)))?;
    
    if !output.status.success() {
        return Err(KvmError::Platform("Failed to open Input Monitoring preferences".to_string()));
    }
    
    log::info!("Opened Input Monitoring preferences");
    Ok(())
}

/// Check firewall configuration (macOS)
#[cfg(target_os = "macos")]
pub fn check_firewall_macos() -> Result<bool> {
    // Check if the application firewall allows incoming connections
    let output = Command::new("sh")
        .arg("-c")
        .arg("/usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate")
        .output();
    
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // If firewall is disabled, we're good
            if stdout.contains("disabled") {
                return Ok(true);
            }
            
            // Check if our app is allowed
            let exe_path = std::env::current_exe()
                .map_err(|e| KvmError::Platform(format!("Failed to get executable path: {}", e)))?;
            
            let app_output = Command::new("sh")
                .arg("-c")
                .arg(format!("/usr/libexec/ApplicationFirewall/socketfilterfw --getappblocked {}", exe_path.to_string_lossy()))
                .output();
            
            match app_output {
                Ok(app_output) => {
                    let app_stdout = String::from_utf8_lossy(&app_output.stdout);
                    Ok(!app_stdout.contains("blocked"))
                }
                Err(_) => Ok(false)
            }
        }
        Err(_) => Ok(false)
    }
}

/// Get firewall configuration instructions (macOS)
#[cfg(target_os = "macos")]
pub fn get_firewall_instructions_macos() -> String {
    "To allow KVM through macOS Firewall:\n\n\
    1. Open System Preferences\n\
    2. Go to Security & Privacy\n\
    3. Click the Firewall tab\n\
    4. Click the lock icon and enter your password\n\
    5. Click 'Firewall Options...'\n\
    6. Click the '+' button\n\
    7. Navigate to and select the KVM application\n\
    8. Ensure 'Allow incoming connections' is selected\n\
    9. Click OK\n\n\
    The application will automatically request firewall access when it starts listening for connections.".to_string()
}

/// Get platform-specific setup instructions
pub fn get_setup_instructions() -> String {
    #[cfg(target_os = "windows")]
    {
        "Windows Setup Instructions:\n\n\
        1. Administrator Privileges:\n\
           - Right-click the application and select 'Run as Administrator'\n\
           - Or configure the application to always run as administrator\n\n\
        2. Firewall Configuration:\n\
           - The application needs to accept incoming connections\n\
           - Windows Firewall will prompt you on first run\n\
           - Or manually configure using the instructions provided\n\n\
        3. Network Discovery:\n\
           - Ensure 'Network Discovery' is enabled in Network Settings\n\
           - Go to Settings > Network & Internet > Sharing options".to_string()
    }
    
    #[cfg(target_os = "macos")]
    {
        "macOS Setup Instructions:\n\n\
        1. Accessibility Permissions:\n\
           - Go to System Preferences > Security & Privacy > Privacy\n\
           - Select 'Accessibility' from the list\n\
           - Click the lock icon and enter your password\n\
           - Check the box next to KVM application\n\n\
        2. Input Monitoring Permissions:\n\
           - Go to System Preferences > Security & Privacy > Privacy\n\
           - Select 'Input Monitoring' from the list\n\
           - Click the lock icon and enter your password\n\
           - Check the box next to KVM application\n\n\
        3. Firewall Configuration:\n\
           - The application will request firewall access automatically\n\
           - Or manually configure in System Preferences > Security & Privacy > Firewall".to_string()
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "Platform not supported".to_string()
    }
}

/// Check if all required permissions are granted
pub fn has_all_permissions() -> Result<bool> {
    let status = check_all_permissions()?;
    
    #[cfg(target_os = "windows")]
    {
        Ok(status.admin && status.firewall_configured)
    }
    
    #[cfg(target_os = "macos")]
    {
        Ok(status.accessibility && status.input_monitoring && status.firewall_configured)
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_check_permissions() {
        let result = check_all_permissions();
        assert!(result.is_ok());
        
        let status = result.unwrap();
        // Just verify the structure is correct
        assert!(status.network); // Network should always be true by default
    }
    
    #[test]
    fn test_get_setup_instructions() {
        let instructions = get_setup_instructions();
        assert!(!instructions.is_empty());
        
        #[cfg(target_os = "windows")]
        assert!(instructions.contains("Administrator"));
        
        #[cfg(target_os = "macos")]
        assert!(instructions.contains("Accessibility"));
    }
    
    #[cfg(target_os = "windows")]
    #[test]
    fn test_check_admin_windows() {
        let result = check_admin_windows();
        assert!(result.is_ok());
        // We can't assert the value since it depends on how the test is run
    }
    
    #[cfg(target_os = "windows")]
    #[test]
    fn test_get_firewall_instructions_windows() {
        let instructions = get_firewall_instructions_windows();
        assert!(instructions.contains("Windows Defender Firewall"));
        assert!(instructions.contains("netsh"));
    }
    
    #[cfg(target_os = "macos")]
    #[test]
    fn test_get_firewall_instructions_macos() {
        let instructions = get_firewall_instructions_macos();
        assert!(instructions.contains("System Preferences"));
        assert!(instructions.contains("Firewall"));
    }
}
