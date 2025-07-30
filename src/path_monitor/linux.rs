//! Linux platform implementation using rtnetlink
//!
//! Uses rtnetlink for monitoring network interface and address changes.

use super::*;
use futures::stream::TryStreamExt;
use netlink_packet_route::address::AddressMessage;
use netlink_packet_route::link::LinkMessage;
use rtnetlink::{new_connection, Handle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

pub struct LinuxMonitor {
    handle: Handle,
    runtime: Arc<Runtime>,
}

impl PlatformMonitor for LinuxMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        let handle = self.handle.clone();
        let runtime = self.runtime.clone();

        runtime.block_on(async {
            let mut interfaces = Vec::new();

            // Get all links
            let mut links = handle.link().get().execute();
            while let Some(msg) = links
                .try_next()
                .await
                .map_err(|e| Error::PlatformError(format!("Failed to get links: {}", e)))?
            {
                if let Some(interface) = parse_link_message(&msg).await {
                    interfaces.push(interface);
                }
            }

            // Get addresses for each interface
            for interface in &mut interfaces {
                let mut addrs = handle.address().get().execute();
                while let Some(msg) = addrs.try_next().await.unwrap_or(None) {
                    if let Some(addr) = parse_address_message(&msg, interface.index) {
                        interface.ips.push(addr);
                    }
                }
            }

            Ok(interfaces)
        })
    }

    fn start_watching(
        &mut self,
        callback: Box<dyn Fn(ChangeEvent) + Send + 'static>,
    ) -> PlatformHandle {
        let _handle = self.handle.clone();
        let runtime = self.runtime.clone();
        let _callback = Arc::new(Mutex::new(callback));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let thread_stop_flag = stop_flag.clone();

        // Spawn a thread to run the async monitoring
        let watcher = thread::spawn(move || {
            runtime.block_on(async {
                // This is a simplified version - actual implementation would
                // subscribe to netlink events and process them
                while !thread_stop_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    // Check for changes and call callback
                }
            });
        });

        Box::new(LinuxMonitorHandle {
            watcher_handle: Some(watcher),
            stop_flag,
        })
    }
}

struct LinuxMonitorHandle {
    watcher_handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl Drop for LinuxMonitorHandle {
    fn drop(&mut self) {
        // Signal the watcher thread to stop
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.watcher_handle.take() {
            // It's generally good practice to join the thread
            // to ensure it has cleaned up properly.
            let _ = handle.join();
        }
    }
}

async fn parse_link_message(msg: &LinkMessage) -> Option<Interface> {
    let mut name = String::new();
    let mut status = Status::Unknown;
    let index = msg.header.index;

    // Parse attributes
    for attr in &msg.attributes {
        use netlink_packet_route::link::LinkAttribute;
        match attr {
            LinkAttribute::IfName(n) => name = n.clone(),
            LinkAttribute::OperState(state) => {
                use netlink_packet_route::link::State;
                status = match state {
                    State::Up => Status::Up,
                    State::Down => Status::Down,
                    _ => Status::Unknown,
                };
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Interface {
        name: name.clone(),
        index,
        ips: Vec::new(),
        status,
        interface_type: detect_interface_type(&name),
        is_expensive: false,
    })
}

fn parse_address_message(msg: &AddressMessage, if_index: u32) -> Option<IpAddr> {
    if msg.header.index != if_index {
        return None;
    }

    for attr in &msg.attributes {
        use netlink_packet_route::address::AddressAttribute;
        match attr {
            AddressAttribute::Address(addr) => {
                // addr is IpAddr, not bytes
                return Some(addr.clone());
            }
            _ => {}
        }
    }

    None
}

fn detect_interface_type(name: &str) -> String {
    if name.starts_with("eth") {
        "ethernet".to_string()
    } else if name.starts_with("wlan") || name.starts_with("wlp") {
        "wifi".to_string()
    } else if name.starts_with("wwan") {
        "cellular".to_string()
    } else if name.starts_with("lo") {
        "loopback".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    let runtime = Arc::new(
        Runtime::new()
            .map_err(|e| Error::PlatformError(format!("Failed to create runtime: {}", e)))?,
    );

    let (conn, handle, _) = runtime.block_on(async {
        new_connection().map_err(|e| {
            Error::PlatformError(format!("Failed to create netlink connection: {}", e))
        })
    })?;

    // Spawn connection handler
    runtime.spawn(conn);

    Ok(Box::new(LinuxMonitor { handle, runtime }))
}
