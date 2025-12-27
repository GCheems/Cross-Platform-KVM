// Example demonstrating the permissions system
// Run with: cargo run --example permissions_demo

use cross_platform_kvm::permissions::{
    check_all_permissions, get_setup_instructions, has_all_permissions,
};

#[tokio::main]
async fn main() {
    println!("=== KVM Permissions Demo ===\n");

    // Check all permissions
    println!("Checking permissions...");
    match check_all_permissions() {
        Ok(status) => {
            println!("\nPermission Status:");
            println!("  Administrator/Root: {}", if status.admin { "✓" } else { "✗" });
            println!("  Accessibility: {}", if status.accessibility { "✓" } else { "✗" });
            println!("  Input Monitoring: {}", if status.input_monitoring { "✓" } else { "✗" });
            println!("  Network: {}", if status.network { "✓" } else { "✗" });
            println!("  Firewall Configured: {}", if status.firewall_configured { "✓" } else { "✗" });
        }
        Err(e) => {
            eprintln!("Error checking permissions: {}", e);
            return;
        }
    }

    // Check if all required permissions are granted
    println!("\nChecking if all required permissions are granted...");
    match has_all_permissions() {
        Ok(has_all) => {
            if has_all {
                println!("✓ All required permissions are granted!");
            } else {
                println!("✗ Some permissions are missing");
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    // Get platform-specific setup instructions
    println!("\n=== Setup Instructions ===\n");
    println!("{}", get_setup_instructions());

    // Platform-specific examples
    #[cfg(target_os = "windows")]
    {
        println!("\n=== Windows-Specific Features ===\n");
        
        use cross_platform_kvm::permissions::{
            check_admin_windows, get_firewall_instructions_windows,
        };
        
        match check_admin_windows() {
            Ok(is_admin) => {
                if is_admin {
                    println!("✓ Running with administrator privileges");
                    
                    // If admin, we could configure firewall automatically
                    println!("\nYou can configure the firewall automatically by calling:");
                    println!("  configure_firewall_windows()");
                } else {
                    println!("✗ Not running with administrator privileges");
                    println!("\nTo configure the firewall, you need admin privileges.");
                }
            }
            Err(e) => {
                eprintln!("Error checking admin status: {}", e);
            }
        }
        
        println!("\n=== Firewall Configuration Instructions ===\n");
        println!("{}", get_firewall_instructions_windows());
    }

    #[cfg(target_os = "macos")]
    {
        println!("\n=== macOS-Specific Features ===\n");
        
        use cross_platform_kvm::permissions::{
            check_accessibility_macos, check_input_monitoring_macos,
            get_firewall_instructions_macos,
        };
        
        match check_accessibility_macos() {
            Ok(has_accessibility) => {
                if has_accessibility {
                    println!("✓ Accessibility permissions granted");
                } else {
                    println!("✗ Accessibility permissions not granted");
                    println!("\nTo grant accessibility permissions:");
                    println!("  1. Call request_accessibility_macos()");
                    println!("  2. Or manually open System Preferences > Security & Privacy > Privacy > Accessibility");
                }
            }
            Err(e) => {
                eprintln!("Error checking accessibility: {}", e);
            }
        }
        
        match check_input_monitoring_macos() {
            Ok(has_input_monitoring) => {
                if has_input_monitoring {
                    println!("✓ Input monitoring permissions granted");
                } else {
                    println!("✗ Input monitoring permissions not granted");
                    println!("\nTo grant input monitoring permissions:");
                    println!("  1. Call request_input_monitoring_macos()");
                    println!("  2. Or manually open System Preferences > Security & Privacy > Privacy > Input Monitoring");
                }
            }
            Err(e) => {
                eprintln!("Error checking input monitoring: {}", e);
            }
        }
        
        println!("\n=== Firewall Configuration Instructions ===\n");
        println!("{}", get_firewall_instructions_macos());
    }

    println!("\n=== Demo Complete ===");
}
