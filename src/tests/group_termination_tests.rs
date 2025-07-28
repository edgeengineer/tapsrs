//! Tests for group-wide connection termination

use crate::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_close_group_closes_all_connections() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connections in background
    let _accept_task = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                // Keep connections alive and echo data
                let mut buf = [0u8; 1024];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) => break, // Connection closed
                        Ok(n) => {
                            if stream.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });

    // Create first connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn1 = preconn.initiate().await.expect("Should connect");

    // Wait for ready
    match conn1.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Clone to create connections in the same group
    let conn2 = conn1.clone_connection().await.expect("Should clone");
    let conn3 = conn1.clone_connection().await.expect("Should clone");

    // Wait for all connections to be ready
    match conn2.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }
    match conn3.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Verify all are in the same group
    let group_id1 = conn1.connection_group_id().await.unwrap();
    let group_id2 = conn2.connection_group_id().await.unwrap();
    let group_id3 = conn3.connection_group_id().await.unwrap();
    assert_eq!(group_id1, group_id2);
    assert_eq!(group_id1, group_id3);

    // Verify group has 3 connections
    assert_eq!(conn1.group_connection_count().await.unwrap(), 3);

    // Close the entire group
    conn1.close_group().await.expect("Should close group");

    // Small delay to ensure closes propagate
    sleep(Duration::from_millis(100)).await;

    // Verify all connections are closed
    assert_eq!(conn1.state().await, ConnectionState::Closed);
    assert_eq!(conn2.state().await, ConnectionState::Closed);
    assert_eq!(conn3.state().await, ConnectionState::Closed);

    // Verify we can't send on any connection
    let msg = Message::from_bytes(b"test");
    assert!(conn1.send(msg.clone()).await.is_err());
    assert!(conn2.send(msg.clone()).await.is_err());
    assert!(conn3.send(msg).await.is_err());
}

#[tokio::test]
async fn test_abort_group_aborts_all_connections() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connections in background
    let _accept_task = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream); // Let connections be aborted
        }
    });

    // Create first connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn1 = preconn.initiate().await.expect("Should connect");

    // Wait for ready
    match conn1.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Clone to create connection in the same group
    let conn2 = conn1.clone_connection().await.expect("Should clone");

    // Wait for second connection to be ready
    match conn2.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Add some data to send buffers to verify they're cleared
    conn1.start_batch().await.unwrap();
    conn1.send(Message::from_bytes(b"pending1")).await.unwrap();

    conn2.start_batch().await.unwrap();
    conn2.send(Message::from_bytes(b"pending2")).await.unwrap();

    // Abort the entire group
    conn1.abort_group().await.expect("Should abort group");

    // Check for ConnectionError event
    match conn1.next_event().await {
        Some(ConnectionEvent::ConnectionError(msg)) => {
            assert!(msg.contains("aborted"));
        }
        other => panic!("Expected ConnectionError event, got {other:?}"),
    }

    // Verify all connections are closed
    assert_eq!(conn1.state().await, ConnectionState::Closed);
    assert_eq!(conn2.state().await, ConnectionState::Closed);

    // Verify we can't send on any connection
    let msg = Message::from_bytes(b"test");
    assert!(conn1.send(msg.clone()).await.is_err());
    assert!(conn2.send(msg).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_close_group_on_ungrouped_connection() {
    tokio::time::timeout(Duration::from_secs(10), async {
        // Start a TCP listener for a real connection
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection in background
        let _server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Keep connection alive and echo data
            let mut buf = [0u8; 1024];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // Connection closed
                    Ok(n) => {
                        if stream.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Create a real connection (not part of a group)
        let preconn = new_preconnection(
            vec![],
            vec![RemoteEndpoint::builder().socket_address(addr).build()],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.expect("Should connect");

        // Wait for ready
        match conn.next_event().await {
            Some(ConnectionEvent::Ready) => {}
            other => panic!("Expected Ready event, got {other:?}"),
        }

        // Verify it's not grouped
        assert!(!conn.is_grouped().await);

        // close_group should just close this connection
        conn.close_group().await.expect("Should close");

        // Check for Closed event
        match conn.next_event().await {
            Some(ConnectionEvent::Closed) => {}
            other => panic!("Expected Closed event, got {other:?}"),
        }

        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_abort_group_on_ungrouped_connection() {
    tokio::time::timeout(Duration::from_secs(10), async {
        // Start a TCP listener for a real connection
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection in background
        let _server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Keep connection alive and echo data
            let mut buf = [0u8; 1024];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // Connection closed
                    Ok(n) => {
                        if stream.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Create a real connection (not part of a group)
        let preconn = new_preconnection(
            vec![],
            vec![RemoteEndpoint::builder().socket_address(addr).build()],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.expect("Should connect");

        // Wait for ready
        match conn.next_event().await {
            Some(ConnectionEvent::Ready) => {}
            other => panic!("Expected Ready event, got {other:?}"),
        }

        // Verify it's not grouped
        assert!(!conn.is_grouped().await);

        // abort_group should just abort this connection
        conn.abort_group().await.expect("Should abort");

        // Check for ConnectionError event
        match conn.next_event().await {
            Some(ConnectionEvent::ConnectionError(msg)) => {
                assert!(msg.contains("aborted"));
            }
            other => panic!("Expected ConnectionError event, got {other:?}"),
        }

        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}
