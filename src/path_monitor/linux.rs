//! Linux platform implementation using rtnetlink
//! 
//! Uses rtnetlink for monitoring network interface and address changes.

use super::*;
use std::thread;
use std::sync::Arc;
use tokio::runtime::Runtime;
use futures::stream::StreamExt;
use rtnetlink::{Handle, new_connection, Error as RtError};
use rtnetlink::packet::rtnl::link::nlas::Nla as LinkNla;
use rtnetlink::packet::rtnl::address::nlas::Nla as AddressNla;
use netlink_packet_route::link::LinkMessage;
use netlink_packet_route::address::AddressMessage;

pub struct LinuxMonitor {
    handle: Handle,
    runtime: Arc<Runtime>,
    watcher_handle: Option<thread::JoinHandle<()>>,
}

impl PlatformMonitor for LinuxMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        let handle = self.handle.clone();
        let runtime = self.runtime.clone();
        
        runtime.block_on(async {
            let mut interfaces = Vec::new();
            
            // Get all links
            let mut links = handle.link().get().execute();
            while let Some(link_msg) = links.next().await {
                match link_msg {
                    Ok(msg) => {
                        if let Some(interface) = parse_link_message(&msg).await {
                            interfaces.push(interface);
                        }
                    }
                    Err(e) => {
                        return Err(Error::PlatformError(format!("Failed to get links: {}", e)));
                    }
                }
            }
            
            // Get addresses for each interface
            for interface in &mut interfaces {
                let mut addrs = handle.address().get().execute();
                while let Some(addr_msg) = addrs.next().await {
                    match addr_msg {
                        Ok(msg) => {
                            if let Some(addr) = parse_address_message(&msg, interface.index) {
                                interface.ips.push(addr);
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
            
            Ok(interfaces)
        })
    }

    fn start_watching(&mut self, callback: Box<dyn Fn(ChangeEvent) + Send + 'static>) -> PlatformHandle {
        let handle = self.handle.clone();
        let runtime = self.runtime.clone();
        let callback = Arc::new(Mutex::new(callback));
        
        // Spawn a thread to run the async monitoring
        let watcher = thread::spawn(move || {
            runtime.block_on(async {
                // Subscribe to link and address events
                let groups = rtnetlink::constants::RTMGRP_LINK | 
                            rtnetlink::constants::RTMGRP_IPV4_IFADDR |
                            rtnetlink::constants::RTMGRP_IPV6_IFADDR;
                
                // This is a simplified version - actual implementation would
                // subscribe to netlink events and process them
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    // Check for changes and call callback
                }
            });
        });
        
        self.watcher_handle = Some(watcher);
        
        Box::new(LinuxMonitorHandle {})
    }
}

struct LinuxMonitorHandle;

impl Drop for LinuxMonitorHandle {
    fn drop(&mut self) {
        // Signal the watcher thread to stop
    }
}

async fn parse_link_message(msg: &LinkMessage) -> Option<Interface> {
    let mut name = String::new();
    let mut status = Status::Unknown;
    let index = msg.header.index;
    
    for nla in &msg.nlas {
        match nla {
            LinkNla::IfName(n) => name = n.clone(),
            LinkNla::OperState(state) => {
                status = match state {
                    6 => Status::Up,    // IF_OPER_UP
                    2 => Status::Down,  // IF_OPER_DOWN
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
    
    for nla in &msg.nlas {
        match nla {
            AddressNla::Address(addr) => {
                match msg.header.family as u16 {
                    2 => { // AF_INET
                        if addr.len() == 4 {
                            let mut bytes = [0u8; 4];
                            bytes.copy_from_slice(addr);
                            return Some(IpAddr::V4(Ipv4Addr::from(bytes)));
                        }
                    }
                    10 => { // AF_INET6
                        if addr.len() == 16 {
                            let mut bytes = [0u8; 16];
                            bytes.copy_from_slice(addr);
                            return Some(IpAddr::V6(Ipv6Addr::from(bytes)));
                        }
                    }
                    _ => {}
                }
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
    let runtime = Arc::new(Runtime::new().map_err(|e| {
        Error::PlatformError(format!("Failed to create runtime: {}", e))
    })?);
    
    let (conn, handle, _) = runtime.block_on(async {
        new_connection().map_err(|e| {
            Error::PlatformError(format!("Failed to create netlink connection: {}", e))
        })
    })?;
    
    // Spawn connection handler
    runtime.spawn(conn);
    
    Ok(Box::new(LinuxMonitor {
        handle,
        runtime,
        watcher_handle: None,
    }))
}