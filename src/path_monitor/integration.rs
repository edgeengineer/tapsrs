//! Integration with Transport Services Connection API
//! 
//! This module shows how path monitoring integrates with the
//! Transport Services Connection establishment and management.

use super::*;
use crate::connection::Connection;
use std::sync::Weak;

/// Extension trait for Connection to support path monitoring
pub trait ConnectionPathMonitoring {
    /// Enable automatic path migration based on network changes
    fn enable_path_monitoring(&self) -> Result<MonitorHandle, Error>;
    
    /// Get current network path information
    fn get_current_path(&self) -> Option<Interface>;
}

/// Path-aware connection manager
pub struct PathAwareConnectionManager {
    monitor: NetworkMonitor,
    connections: Arc<Mutex<Vec<Weak<Connection>>>>,
}

impl PathAwareConnectionManager {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            monitor: NetworkMonitor::new()?,
            connections: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// Register a connection for path monitoring
    pub fn register_connection(&self, conn: Weak<Connection>) {
        self.connections.lock().unwrap().push(conn);
    }
    
    /// Start monitoring and managing paths for all connections
    pub fn start_monitoring(&self) -> MonitorHandle {
        let connections = self.connections.clone();
        
        self.monitor.watch_changes(move |event| {
            let mut conns = connections.lock().unwrap();
            
            // Clean up dead weak references
            conns.retain(|conn| conn.strong_count() > 0);
            
            // Handle the event for each connection
            for conn_weak in conns.iter() {
                if let Some(_conn) = conn_weak.upgrade() {
                    match &event {
                        ChangeEvent::PathChanged { description } => {
                            log::info!("Path changed for connection: {}", description);
                            // TODO: Trigger connection migration if supported
                        }
                        ChangeEvent::Removed(interface) => {
                            log::warn!("Interface {} removed", interface.name);
                            // TODO: Check if this affects the connection
                        }
                        ChangeEvent::Modified { old, new } => {
                            if old.status == Status::Up && new.status == Status::Down {
                                log::warn!("Interface {} went down", new.name);
                                // TODO: Trigger failover if this is the current path
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
    }
    
    /// Get available paths for a connection
    pub fn get_available_paths(&self) -> Result<Vec<Interface>, Error> {
        self.monitor.list_interfaces()
    }
    
    /// Select best path based on connection requirements
    pub fn select_best_path(
        &self,
        prefer_wifi: bool,
        avoid_expensive: bool,
    ) -> Result<Option<Interface>, Error> {
        let interfaces = self.monitor.list_interfaces()?;
        
        let mut candidates: Vec<_> = interfaces
            .into_iter()
            .filter(|iface| {
                iface.status == Status::Up &&
                !iface.ips.is_empty() &&
                iface.interface_type != "loopback"
            })
            .collect();
        
        if avoid_expensive {
            candidates.retain(|iface| !iface.is_expensive);
        }
        
        if prefer_wifi {
            // Sort to put wifi interfaces first
            candidates.sort_by(|a, b| {
                match (&a.interface_type[..], &b.interface_type[..]) {
                    ("wifi", "wifi") => std::cmp::Ordering::Equal,
                    ("wifi", _) => std::cmp::Ordering::Less,
                    (_, "wifi") => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }
        
        Ok(candidates.into_iter().next())
    }
}

/// Multipath policy implementation based on RFC 9622
#[derive(Debug, Clone)]
pub enum MultipathMode {
    /// Don't use multiple paths
    Disabled,
    /// Actively use multiple paths
    Active,
    /// Use multiple paths if peer requests
    Passive,
}

/// Path selection preferences
#[derive(Debug, Clone)]
pub struct PathPreferences {
    /// Prefer specific interface types
    pub preferred_types: Vec<String>,
    /// Avoid expensive (metered) connections
    pub avoid_expensive: bool,
    /// Minimum number of paths to maintain
    pub min_paths: usize,
    /// Maximum number of paths to use
    pub max_paths: usize,
}

impl Default for PathPreferences {
    fn default() -> Self {
        Self {
            preferred_types: vec!["wifi".to_string(), "ethernet".to_string()],
            avoid_expensive: true,
            min_paths: 1,
            max_paths: 2,
        }
    }
}