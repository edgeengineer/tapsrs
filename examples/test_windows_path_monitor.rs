//! Test example for Windows path monitoring
//! 
//! This example tests the Windows path monitor implementation

use transport_services::path_monitor::{NetworkMonitor, ChangeEvent};
use std::time::Duration;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("Creating network monitor...");
    let monitor = NetworkMonitor::new()?;
    
    // List current interfaces
    println!("\nCurrent network interfaces:");
    let interfaces = monitor.list_interfaces()?;
    for interface in &interfaces {
        println!("- {} (index: {})", interface.name, interface.index);
        println!("  Type: {}", interface.interface_type);
        println!("  Status: {:?}", interface.status);
        println!("  IPs: {:?}", interface.ips);
        println!("  Expensive: {}", interface.is_expensive);
    }
    
    // Start monitoring for changes
    println!("\nStarting network change monitoring...");
    let _handle = monitor.watch_changes(|event| {
        match event {
            ChangeEvent::Added(interface) => {
                println!("Interface added: {} ({})", interface.name, interface.interface_type);
            }
            ChangeEvent::Removed(interface) => {
                println!("Interface removed: {} ({})", interface.name, interface.interface_type);
            }
            ChangeEvent::Modified { old, new } => {
                println!("Interface modified: {}", new.name);
                if old.status != new.status {
                    println!("  Status changed: {:?} -> {:?}", old.status, new.status);
                }
                if old.ips != new.ips {
                    println!("  IPs changed: {:?} -> {:?}", old.ips, new.ips);
                }
            }
            ChangeEvent::PathChanged { description } => {
                println!("Path changed: {}", description);
            }
        }
    });
    
    println!("Monitoring for 30 seconds... (disable/enable network adapters to see changes)");
    thread::sleep(Duration::from_secs(30));
    
    println!("\nStopping monitor...");
    Ok(())
}