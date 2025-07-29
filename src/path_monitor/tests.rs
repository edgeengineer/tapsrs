//! Tests for the path monitor module

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_create_network_monitor() {
        // This test might fail on unsupported platforms
        match NetworkMonitor::new() {
            Ok(_monitor) => {
                // Successfully created monitor
            }
            Err(Error::NotSupported) => {
                // Platform not supported, which is expected for some platforms
            }
            Err(e) => {
                panic!("Unexpected error creating NetworkMonitor: {:?}", e);
            }
        }
    }

    #[test]
    fn test_list_interfaces() {
        match NetworkMonitor::new() {
            Ok(monitor) => {
                match monitor.list_interfaces() {
                    Ok(interfaces) => {
                        // Should have at least a loopback interface on most systems
                        assert!(!interfaces.is_empty(), "No interfaces found");
                        
                        // Check that interfaces have required fields
                        for interface in interfaces {
                            assert!(!interface.name.is_empty());
                            // Most systems have at least one IP on loopback
                            if interface.interface_type == "loopback" {
                                assert!(!interface.ips.is_empty());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list interfaces: {:?}", e);
                    }
                }
            }
            Err(Error::NotSupported) => {
                // Skip test on unsupported platforms
            }
            Err(e) => {
                panic!("Failed to create monitor: {:?}", e);
            }
        }
    }

    #[test]
    fn test_monitor_handle_drop() {
        // Test that the monitor handle properly stops monitoring when dropped
        match NetworkMonitor::new() {
            Ok(monitor) => {
                let events = Arc::new(Mutex::new(Vec::new()));
                let events_clone = events.clone();
                
                {
                    let _handle = monitor.watch_changes(move |event| {
                        events_clone.lock().unwrap().push(format!("{:?}", event));
                    });
                    // Handle is dropped here
                }
                
                // Give some time for cleanup
                thread::sleep(Duration::from_millis(100));
                
                // No more events should be received after handle is dropped
                let initial_count = events.lock().unwrap().len();
                thread::sleep(Duration::from_millis(100));
                let final_count = events.lock().unwrap().len();
                
                assert_eq!(initial_count, final_count, "Events received after handle dropped");
            }
            Err(Error::NotSupported) => {
                // Skip test on unsupported platforms
            }
            Err(e) => {
                panic!("Failed to create monitor: {:?}", e);
            }
        }
    }

    #[test]
    fn test_interface_status() {
        match NetworkMonitor::new() {
            Ok(monitor) => {
                if let Ok(interfaces) = monitor.list_interfaces() {
                    for interface in interfaces {
                        // Status should be one of the defined values
                        match interface.status {
                            Status::Up | Status::Down | Status::Unknown => {
                                // Valid status
                            }
                        }
                    }
                }
            }
            Err(Error::NotSupported) => {
                // Skip test on unsupported platforms
            }
            Err(_) => {
                // Ignore other errors in this test
            }
        }
    }
}