//! Example that demonstrates watching for network path changes
//!
//! This example starts monitoring network changes and reports them in real-time

use std::thread;
use std::time::Duration;
use transport_services::path_monitor::{ChangeEvent, NetworkMonitor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Starting network path monitor...");
    println!("Try disconnecting/connecting WiFi or changing networks to see events");
    println!("Running for 30 seconds...\n");

    // Create a network monitor
    let monitor = NetworkMonitor::new()?;

    // Start watching for changes
    let _handle = monitor.watch_changes(|event| {
        println!(
            "[{}] Network event: {:?}",
            chrono::Local::now().format("%H:%M:%S"),
            event
        );

        match event {
            ChangeEvent::PathChanged { description } => {
                println!("  → {}", description);
            }
            ChangeEvent::Added(interface) => {
                println!(
                    "  → Interface {} added (type: {}, expensive: {})",
                    interface.name,
                    interface.interface_type,
                    if interface.is_expensive { "yes" } else { "no" }
                );
            }
            ChangeEvent::Removed(interface) => {
                println!("  → Interface {} removed", interface.name);
            }
            ChangeEvent::Modified { old, new } => {
                println!("  → Interface {} modified:", new.name);
                if old.status != new.status {
                    println!("    Status: {:?} → {:?}", old.status, new.status);
                }
                if old.ips != new.ips {
                    println!("    IPs changed");
                }
            }
        }
        println!();
    });

    // Keep the program running for 30 seconds
    thread::sleep(Duration::from_secs(30));

    println!("Monitoring complete.");
    Ok(())
}
