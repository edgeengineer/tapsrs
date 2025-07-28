//! Tests for connection property updates and group-wide property synchronization

use crate::*;
use tokio::net::TcpListener;
use std::time::Duration;

#[tokio::test]
async fn test_group_wide_property_update() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Accept connections in background
    let _accept_task = tokio::spawn(async move {
        for _ in 0..3 {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    
    // Create first connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .socket_address(addr)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    
    let conn1 = preconn.initiate().await.expect("Should connect");
    
    // Wait for ready
    match conn1.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    // Clone to create connections in the same group
    let conn2 = conn1.clone_connection().await.expect("Should clone");
    let conn3 = conn1.clone_connection().await.expect("Should clone");
    
    // Wait for clones to be ready
    match conn2.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    match conn3.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    // Set a property that should be shared across the group
    let new_profile = CapacityProfile::LowLatencyInteractive;
    conn1.set_property("connCapacityProfile", 
        ConnectionProperty::ConnCapacityProfile(new_profile))
        .await
        .expect("Should set property");
    
    // Small delay to ensure property propagation
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify all connections have the updated property
    let props1 = conn1.get_properties().await;
    let props2 = conn2.get_properties().await;
    let props3 = conn3.get_properties().await;
    
    if let Some(ConnectionProperty::ConnCapacityProfile(profile1)) = props1.get("connCapacityProfile") {
        assert_eq!(*profile1, new_profile);
    } else {
        panic!("Property not found on conn1");
    }
    
    if let Some(ConnectionProperty::ConnCapacityProfile(profile2)) = props2.get("connCapacityProfile") {
        assert_eq!(*profile2, new_profile);
    } else {
        panic!("Property not found on conn2");
    }
    
    if let Some(ConnectionProperty::ConnCapacityProfile(profile3)) = props3.get("connCapacityProfile") {
        assert_eq!(*profile3, new_profile);
    } else {
        panic!("Property not found on conn3");
    }
}

#[tokio::test]
async fn test_conn_priority_not_shared() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Accept connections in background
    let _accept_task = tokio::spawn(async move {
        for _ in 0..2 {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    
    // Create first connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .socket_address(addr)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    
    let conn1 = preconn.initiate().await.expect("Should connect");
    
    // Wait for ready
    match conn1.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    // Clone to create connection in the same group
    let conn2 = conn1.clone_connection().await.expect("Should clone");
    
    // Wait for clone to be ready
    match conn2.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    // Set different priorities on each connection
    conn1.set_property("connPriority", ConnectionProperty::ConnPriority(10))
        .await
        .expect("Should set priority");
        
    conn2.set_property("connPriority", ConnectionProperty::ConnPriority(50))
        .await
        .expect("Should set priority");
    
    // Verify priorities are NOT shared (connPriority is per-connection)
    if let Some(ConnectionProperty::ConnPriority(priority1)) = conn1.get_property("connPriority").await {
        assert_eq!(priority1, 10);
    } else {
        panic!("Priority not found on conn1");
    }
    
    if let Some(ConnectionProperty::ConnPriority(priority2)) = conn2.get_property("connPriority").await {
        assert_eq!(priority2, 50);
    } else {
        panic!("Priority not found on conn2");
    }
}

#[tokio::test]
async fn test_keepalive_timeout_setting() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Accept connection in background
    let _accept_task = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    
    // Create connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .socket_address(addr)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    
    let conn = preconn.initiate().await.expect("Should connect");
    
    // Wait for ready
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    // Set keep-alive timeout
    let keepalive_duration = Duration::from_secs(30);
    conn.set_property("keepAliveTimeout", 
        ConnectionProperty::KeepAliveTimeout(TimeoutValue::Duration(keepalive_duration)))
        .await
        .expect("Should set keep-alive");
    
    // Verify it was set
    if let Some(ConnectionProperty::KeepAliveTimeout(timeout)) = conn.get_property("keepAliveTimeout").await {
        match timeout {
            TimeoutValue::Duration(d) => assert_eq!(d, keepalive_duration),
            _ => panic!("Expected Duration timeout"),
        }
    } else {
        panic!("Keep-alive property not found");
    }
    
    // Disable keep-alive
    conn.set_property("keepAliveTimeout", 
        ConnectionProperty::KeepAliveTimeout(TimeoutValue::Disabled))
        .await
        .expect("Should disable keep-alive");
    
    // Verify it was disabled
    if let Some(ConnectionProperty::KeepAliveTimeout(timeout)) = conn.get_property("keepAliveTimeout").await {
        match timeout {
            TimeoutValue::Disabled => {}, // Good
            _ => panic!("Expected Disabled timeout"),
        }
    } else {
        panic!("Keep-alive property not found");
    }
}

#[tokio::test]
async fn test_connection_timeout_setting() {
    // Create a connection without actually connecting
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    
    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );
    
    // Set connection timeout
    let timeout_duration = Duration::from_secs(60);
    conn.set_property("connTimeout", 
        ConnectionProperty::ConnTimeout(TimeoutValue::Duration(timeout_duration)))
        .await
        .expect("Should set timeout");
    
    // Verify it was set
    if let Some(ConnectionProperty::ConnTimeout(timeout)) = conn.get_property("connTimeout").await {
        match timeout {
            TimeoutValue::Duration(d) => assert_eq!(d, timeout_duration),
            _ => panic!("Expected Duration timeout"),
        }
    } else {
        panic!("Connection timeout property not found");
    }
}