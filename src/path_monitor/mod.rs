//! Network path monitoring implementation for Transport Services
//! 
//! This module provides cross-platform network interface and path monitoring,
//! allowing applications to track network changes and adapt connections accordingly.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Platform-specific implementations
#[cfg(target_vendor = "apple")]
mod apple;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "android")]
mod android;

pub mod integration;

// Common types across platforms
#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,              // e.g., "en0", "eth0"
    pub index: u32,                // Interface index
    pub ips: Vec<IpAddr>,          // List of assigned IPs
    pub status: Status,            // Up/Down/Unknown
    pub interface_type: String,    // e.g., "wifi", "ethernet", "cellular"
    pub is_expensive: bool,        // e.g., metered like cellular
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Up,
    Down,
    Unknown,
}

#[derive(Debug)]
pub enum ChangeEvent {
    Added(Interface),
    Removed(Interface),
    Modified { old: Interface, new: Interface },
    PathChanged { description: String },  // Generic path change info
}

// The main API struct
pub struct NetworkMonitor {
    // Internal state, e.g., Arc<Mutex<PlatformSpecificImpl>>
    inner: Arc<Mutex<Box<dyn PlatformMonitor + Send + Sync>>>,
}

impl NetworkMonitor {
    /// Create a new monitor
    pub fn new() -> Result<Self, Error> {
        let inner = create_platform_impl()?;
        Ok(Self { 
            inner: Arc::new(Mutex::new(inner))
        })
    }

    /// List current interfaces synchronously
    pub fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        let guard = self.inner.lock().unwrap();
        guard.list_interfaces()
    }

    /// Start watching for changes; returns a handle to stop
    pub fn watch_changes<F>(&self, callback: F) -> MonitorHandle
    where
        F: Fn(ChangeEvent) + Send + 'static,
    {
        let mut guard = self.inner.lock().unwrap();
        let handle = guard.start_watching(Box::new(callback));
        MonitorHandle { _inner: handle }  // RAII to stop on drop
    }
}

// Handle to stop monitoring (drops the watcher)
pub struct MonitorHandle {
    _inner: PlatformHandle,  // Platform-specific drop logic
}

#[derive(Debug)]
pub enum Error {
    PlatformError(String),
    PermissionDenied,
    NotSupported,
    // etc.
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            Error::PermissionDenied => write!(f, "Permission denied"),
            Error::NotSupported => write!(f, "Operation not supported on this platform"),
        }
    }
}

impl std::error::Error for Error {}

// Platform abstraction trait
trait PlatformMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error>;
    fn start_watching(&mut self, callback: Box<dyn Fn(ChangeEvent) + Send + 'static>) -> PlatformHandle;
}

type PlatformHandle = Box<dyn Drop + Send>;  // Platform-specific handle

// Platform implementation factory
#[cfg(target_vendor = "apple")]
fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    apple::create_platform_impl()
}

#[cfg(target_os = "linux")]
fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    linux::create_platform_impl()
}

#[cfg(target_os = "windows")]
fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    windows::create_platform_impl()
}

#[cfg(target_os = "android")]
fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    android::create_platform_impl()
}

#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "windows",
    target_os = "android"
)))]
fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Err(Error::NotSupported)
}

#[cfg(test)]
mod tests;