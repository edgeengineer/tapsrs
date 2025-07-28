//! Tests for TCP-specific connection properties (RFC 8.2)

use crate::*;
use tokio::net::TcpListener;
use std::time::Duration;

async fn create_test_connection() -> Connection {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Accept connection in background
    tokio::spawn(async move {
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
    
    conn
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_user_timeout_enabled_default() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Check default value is false
        if let Some(ConnectionProperty::TcpUserTimeoutEnabled(enabled)) = 
            conn.get_property("tcp.userTimeoutEnabled").await {
            assert!(!enabled, "TCP User Timeout should be disabled by default");
        } else {
            panic!("tcp.userTimeoutEnabled property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_user_timeout_changeable_default() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Check default value is true
        if let Some(ConnectionProperty::TcpUserTimeoutChangeable(changeable)) = 
            conn.get_property("tcp.userTimeoutChangeable").await {
            assert!(changeable, "TCP User Timeout should be changeable by default");
        } else {
            panic!("tcp.userTimeoutChangeable property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_user_timeout_value_not_set_by_default() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Check that tcp.userTimeoutValue is not set by default
        let value = conn.get_property("tcp.userTimeoutValue").await;
        assert!(value.is_none() || 
            matches!(value, Some(ConnectionProperty::TcpUserTimeoutValue(None))),
            "tcp.userTimeoutValue should not be set by default");
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_enable_tcp_user_timeout() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Enable TCP User Timeout
        conn.set_property("tcp.userTimeoutEnabled", 
            ConnectionProperty::TcpUserTimeoutEnabled(true))
            .await
            .expect("Should set property");
        
        // Verify it was enabled
        if let Some(ConnectionProperty::TcpUserTimeoutEnabled(enabled)) = 
            conn.get_property("tcp.userTimeoutEnabled").await {
            assert!(enabled, "TCP User Timeout should be enabled");
        } else {
            panic!("tcp.userTimeoutEnabled property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_tcp_user_timeout_value() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Set TCP User Timeout value
        let timeout_value = Duration::from_secs(30);
        conn.set_property("tcp.userTimeoutValue", 
            ConnectionProperty::TcpUserTimeoutValue(Some(timeout_value)))
            .await
            .expect("Should set property");
        
        // Verify it was set
        if let Some(ConnectionProperty::TcpUserTimeoutValue(value)) = 
            conn.get_property("tcp.userTimeoutValue").await {
            assert_eq!(value, Some(timeout_value), "TCP User Timeout value should be set");
        } else {
            panic!("tcp.userTimeoutValue property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conn_timeout_affects_tcp_user_timeout_changeable() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Verify tcp.userTimeoutChangeable is true initially
        if let Some(ConnectionProperty::TcpUserTimeoutChangeable(changeable)) = 
            conn.get_property("tcp.userTimeoutChangeable").await {
            assert!(changeable, "Should be changeable initially");
        } else {
            panic!("tcp.userTimeoutChangeable property not found");
        }
        
        // Set connTimeout
        conn.set_property("connTimeout", 
            ConnectionProperty::ConnTimeout(TimeoutValue::Duration(Duration::from_secs(60))))
            .await
            .expect("Should set property");
        
        // Verify tcp.userTimeoutChangeable became false
        if let Some(ConnectionProperty::TcpUserTimeoutChangeable(changeable)) = 
            conn.get_property("tcp.userTimeoutChangeable").await {
            assert!(!changeable, "Should not be changeable after setting connTimeout");
        } else {
            panic!("tcp.userTimeoutChangeable property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_properties_full_workflow() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Enable TCP User Timeout
        conn.set_property("tcp.userTimeoutEnabled", 
            ConnectionProperty::TcpUserTimeoutEnabled(true))
            .await
            .expect("Should enable TCP UTO");
        
        // Set a timeout value
        let timeout_value = Duration::from_secs(45);
        conn.set_property("tcp.userTimeoutValue", 
            ConnectionProperty::TcpUserTimeoutValue(Some(timeout_value)))
            .await
            .expect("Should set timeout value");
        
        // Verify both are set
        let props = conn.get_properties().await;
        
        if let Some(ConnectionProperty::TcpUserTimeoutEnabled(enabled)) = 
            props.get("tcp.userTimeoutEnabled") {
            assert!(*enabled, "TCP UTO should be enabled");
        } else {
            panic!("tcp.userTimeoutEnabled not found");
        }
        
        if let Some(ConnectionProperty::TcpUserTimeoutValue(value)) = 
            props.get("tcp.userTimeoutValue") {
            assert_eq!(*value, Some(timeout_value), "TCP UTO value should match");
        } else {
            panic!("tcp.userTimeoutValue not found");
        }
        
        if let Some(ConnectionProperty::TcpUserTimeoutChangeable(changeable)) = 
            props.get("tcp.userTimeoutChangeable") {
            assert!(*changeable, "Should still be changeable");
        } else {
            panic!("tcp.userTimeoutChangeable not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_properties_on_closed_connection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Close the connection
        conn.close().await.expect("Should close");
        
        // TCP properties should still be accessible but operations may not have effect
        conn.set_property("tcp.userTimeoutEnabled", 
            ConnectionProperty::TcpUserTimeoutEnabled(true))
            .await
            .expect("Should be able to set property on closed connection");
        
        // Verify property was set
        if let Some(ConnectionProperty::TcpUserTimeoutEnabled(enabled)) = 
            conn.get_property("tcp.userTimeoutEnabled").await {
            assert!(enabled, "Property should be set even on closed connection");
        } else {
            panic!("tcp.userTimeoutEnabled property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_disable_conn_timeout_does_not_affect_tcp_changeable() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Set connTimeout to Disabled (should not affect tcp.userTimeoutChangeable)
        conn.set_property("connTimeout", 
            ConnectionProperty::ConnTimeout(TimeoutValue::Disabled))
            .await
            .expect("Should set property");
        
        // Verify tcp.userTimeoutChangeable is still true
        if let Some(ConnectionProperty::TcpUserTimeoutChangeable(changeable)) = 
            conn.get_property("tcp.userTimeoutChangeable").await {
            assert!(changeable, "Should remain changeable when connTimeout is Disabled");
        } else {
            panic!("tcp.userTimeoutChangeable property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tcp_properties_group_synchronization() {
    tokio::time::timeout(Duration::from_secs(5), async {
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
        
        // Set TCP property on first connection
        conn1.set_property("tcp.userTimeoutEnabled", 
            ConnectionProperty::TcpUserTimeoutEnabled(true))
            .await
            .expect("Should set property");
        
        // Small delay to ensure property propagation
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify it's synchronized to second connection
        if let Some(ConnectionProperty::TcpUserTimeoutEnabled(enabled)) = 
            conn2.get_property("tcp.userTimeoutEnabled").await {
            assert!(enabled, "TCP property should be synchronized across group");
        } else {
            panic!("tcp.userTimeoutEnabled property not found on conn2");
        }
    }).await.expect("Test should complete within timeout");
}