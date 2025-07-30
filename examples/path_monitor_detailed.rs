//! Detailed example demonstrating network path monitoring
//!
//! This example shows:
//! - Detecting expensive (metered) interfaces
//! - Interface indices
//! - Detailed interface information

use std::net::IpAddr;
use transport_services::path_monitor::{NetworkMonitor, Status};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create a network monitor
    let monitor = NetworkMonitor::new()?;

    println!("Network Interface Details");
    println!("========================\n");

    // List current interfaces with detailed information
    let interfaces = monitor.list_interfaces()?;

    // Group interfaces by type
    let mut by_type = std::collections::HashMap::<String, Vec<_>>::new();
    for interface in &interfaces {
        by_type
            .entry(interface.interface_type.clone())
            .or_default()
            .push(interface);
    }

    // Display interfaces grouped by type
    for (iface_type, ifaces) in by_type {
        println!("## {} Interfaces", iface_type.to_uppercase());
        println!();

        for interface in ifaces {
            println!("Interface: {} (index: {})", interface.name, interface.index);
            println!("  Status: {:?}", interface.status);
            println!(
                "  Expensive: {}",
                if interface.is_expensive {
                    "Yes ⚠️"
                } else {
                    "No ✓"
                }
            );

            if !interface.ips.is_empty() {
                println!("  IP Addresses:");
                for ip in &interface.ips {
                    match ip {
                        IpAddr::V4(v4) => println!("    - IPv4: {}", v4),
                        IpAddr::V6(v6) => {
                            // Skip link-local IPv6 for brevity
                            if !v6.segments()[0] == 0xfe80 {
                                println!("    - IPv6: {}", v6);
                            }
                        }
                    }
                }
            }
            println!();
        }
    }

    // Summary statistics
    println!("## Summary");
    println!();

    let active_count = interfaces.iter().filter(|i| i.status == Status::Up).count();

    let expensive_count = interfaces
        .iter()
        .filter(|i| i.is_expensive && i.status == Status::Up)
        .count();

    println!("Total interfaces: {}", interfaces.len());
    println!("Active interfaces: {}", active_count);
    println!(
        "Expensive interfaces: {} {}",
        expensive_count,
        if expensive_count > 0 { "⚠️" } else { "" }
    );

    // Find preferred interface for internet connectivity
    let preferred = interfaces
        .iter()
        .filter(|i| {
            i.status == Status::Up
                && !i.is_expensive
                && !i.ips.is_empty()
                && (i.interface_type == "wifi" || i.interface_type == "ethernet")
        })
        .next();

    if let Some(pref) = preferred {
        println!(
            "\nPreferred interface for internet: {} ({})",
            pref.name, pref.interface_type
        );
    }

    Ok(())
}
