//! Example demonstrating network path monitoring
//!
//! This example shows how to use the NetworkMonitor to:
//! 1. List current network interfaces
//! 2. Monitor for network changes

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use transport_services::path_monitor::{ChangeEvent, NetworkMonitor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create a network monitor
    let monitor = NetworkMonitor::new()?;

    // List current interfaces
    println!("Current network interfaces:");
    println!("==========================");

    let interfaces = monitor.list_interfaces()?;
    for interface in interfaces {
        println!("\nInterface: {}", interface.name);
        println!("  Index: {}", interface.index);
        println!("  Type: {}", interface.interface_type);
        println!("  Status: {:?}", interface.status);
        println!("  Expensive: {}", interface.is_expensive);
        println!("  IP Addresses:");
        for ip in &interface.ips {
            println!("    - {}", ip);
        }
    }

    println!("\n\nMonitoring for network changes (press Ctrl+C to stop)...");
    println!("=========================================================");

    // Set up monitoring
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Handle Ctrl+C
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // Start monitoring for changes
    let _handle = monitor.watch_changes(|event| match event {
        ChangeEvent::Added(interface) => {
            println!(
                "\n[ADDED] Interface: {} ({})",
                interface.name, interface.interface_type
            );
            for ip in &interface.ips {
                println!("  - IP: {}", ip);
            }
        }
        ChangeEvent::Removed(interface) => {
            println!(
                "\n[REMOVED] Interface: {} ({})",
                interface.name, interface.interface_type
            );
        }
        ChangeEvent::Modified { old, new } => {
            println!("\n[MODIFIED] Interface: {}", new.name);
            if old.status != new.status {
                println!("  - Status: {:?} -> {:?}", old.status, new.status);
            }
            if old.ips != new.ips {
                println!("  - IPs changed");
                println!("    Old: {:?}", old.ips);
                println!("    New: {:?}", new.ips);
            }
        }
        ChangeEvent::PathChanged { description } => {
            println!("\n[PATH CHANGE] {}", description);
        }
    });

    // Keep running until interrupted
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }

    println!("\nStopping monitor...");
    Ok(())
}
