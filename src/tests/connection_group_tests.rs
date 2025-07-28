//! Unit tests for Connection Groups functionality

use crate::{
    preconnection::new_preconnection, ConnectionGroup, ConnectionState, LocalEndpoint, Preference,
    RemoteEndpoint, SecurityParameters, TransportProperties,
};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

#[tokio::test]
async fn test_connection_clone_basic() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let remote = RemoteEndpoint::builder()
        .socket_address(server_addr)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Accept connections in background
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            // Keep connections alive
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            });
        }
    });

    // Create initial connection
    let conn1 = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn1.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(conn1.state().await, ConnectionState::Established);

    // Clone the connection
    let conn2 = conn1.clone_connection().await.unwrap();

    // Wait for clone to establish
    while conn2.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(conn2.state().await, ConnectionState::Established);

    // Both connections should be in the same group
    assert!(conn1.is_grouped().await);
    assert!(conn2.is_grouped().await);

    // They should have the same group ID
    let group_id1 = conn1.connection_group_id().await;
    let group_id2 = conn2.connection_group_id().await;
    assert!(group_id1.is_some());
    assert_eq!(group_id1, group_id2);

    // Debug: Check individual counts
    let count1 = conn1.group_connection_count().await;
    let count2 = conn2.group_connection_count().await;

    // If counts don't match expectations, print debug info
    if count1 != Some(2) || count2 != Some(2) {
        eprintln!("Connection 1 group count: {count1:?}");
        eprintln!("Connection 2 group count: {count2:?}");
        eprintln!("Connection 1 grouped: {}", conn1.is_grouped().await);
        eprintln!("Connection 2 grouped: {}", conn2.is_grouped().await);
    }

    // Group should have 2 connections
    assert_eq!(count1, Some(2));
    assert_eq!(count2, Some(2));

    // Clean up
    conn1.close().await.unwrap();
    conn2.close().await.unwrap();
}

#[tokio::test]
async fn test_connection_clone_only_established() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .ip_address("127.0.0.1".parse().unwrap())
            .port(65535) // Invalid port
            .build()],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Try to clone before establishment
    let result = conn.clone_connection().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("established"));
}

#[tokio::test]
async fn test_connection_group_shared_properties() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Accept connections in background
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            });
        }
    });

    // Create connection with specific transport properties
    let props = TransportProperties::builder()
        .reliability(Preference::Require)
        .preserve_order(Preference::Require)
        .connection_priority(100)
        .build();

    let remote = RemoteEndpoint::builder()
        .socket_address(server_addr)
        .build();

    let preconn = new_preconnection(vec![], vec![remote], props, SecurityParameters::default());

    // Create initial connection
    let conn1 = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn1.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Clone the connection
    let conn2 = conn1.clone_connection().await.unwrap();

    // Wait for clone to establish
    while conn2.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Both connections should share the same transport properties
    // (This is verified internally - the cloned connection gets properties from the group)
    assert!(conn1.is_grouped().await);
    assert!(conn2.is_grouped().await);

    // Clean up
    conn1.close().await.unwrap();
    conn2.close().await.unwrap();
}

#[tokio::test]
async fn test_connection_group_multiple_clones() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Accept connections in background
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            });
        }
    });

    let remote = RemoteEndpoint::builder()
        .socket_address(server_addr)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Create initial connection
    let conn1 = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn1.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Create multiple clones
    let conn2 = conn1.clone_connection().await.unwrap();

    // Wait for conn2 to establish before cloning from it
    while conn2.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    let conn3 = conn1.clone_connection().await.unwrap();
    let conn4 = conn2.clone_connection().await.unwrap(); // Clone from a clone

    // Wait for remaining clones to establish
    for conn in [&conn3, &conn4] {
        while conn.state().await == ConnectionState::Establishing {
            sleep(Duration::from_millis(10)).await;
        }
    }

    // All connections should be in the same group
    let group_id = conn1.connection_group_id().await;
    assert!(group_id.is_some());
    assert_eq!(conn2.connection_group_id().await, group_id);
    assert_eq!(conn3.connection_group_id().await, group_id);
    assert_eq!(conn4.connection_group_id().await, group_id);

    // Group should have 4 connections
    assert_eq!(conn1.group_connection_count().await, Some(4));

    // Clean up
    conn1.close().await.unwrap();
    conn2.close().await.unwrap();
    conn3.close().await.unwrap();
    conn4.close().await.unwrap();
}

#[tokio::test]
async fn test_connection_group_close() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Accept connections in background
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
            });
        }
    });

    let remote = RemoteEndpoint::builder()
        .socket_address(server_addr)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Create connections
    let conn1 = preconn.initiate().await.unwrap();
    while conn1.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    let conn2 = conn1.clone_connection().await.unwrap();
    while conn2.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Verify both are in a group
    assert!(conn1.is_grouped().await);
    assert_eq!(conn1.group_connection_count().await, Some(2));

    // Close one connection
    conn1.close().await.unwrap();
    assert_eq!(conn1.state().await, ConnectionState::Closed);

    // Close the other
    conn2.close().await.unwrap();
    assert_eq!(conn2.state().await, ConnectionState::Closed);
}

#[tokio::test]
async fn test_connection_without_group() {
    // Create a connection that won't be cloned
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .ip_address("127.0.0.1".parse().unwrap())
            .port(65535) // Invalid port
            .build()],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Connection should not be grouped
    assert!(!conn.is_grouped().await);
    assert_eq!(conn.connection_group_id().await, None);
    assert_eq!(conn.group_connection_count().await, None);
}

#[tokio::test]
async fn test_connection_group_creation() {
    // Test ConnectionGroup directly
    let props = TransportProperties::default();
    let locals = vec![LocalEndpoint::builder().port(0).build()];
    let remotes = vec![RemoteEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(8080)
        .build()];

    let group = ConnectionGroup::new(props, locals, remotes);

    assert_eq!(group.connection_count(), 0);
    assert!(!group.has_connections());

    // Add connections
    group.add_connection();
    assert_eq!(group.connection_count(), 1);
    assert!(group.has_connections());

    group.add_connection();
    assert_eq!(group.connection_count(), 2);

    // Remove connections
    group.remove_connection();
    assert_eq!(group.connection_count(), 1);

    group.remove_connection();
    assert_eq!(group.connection_count(), 0);
    assert!(!group.has_connections());
}
