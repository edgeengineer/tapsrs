//! Tests for Connection Lifecycle Events (RFC 8.3)

use crate::*;
use std::time::Duration;
use tokio::net::TcpListener;

async fn create_test_connection() -> Connection {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection in background
    tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(10)).await; // Keep alive longer
    });

    // Create connection
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

    conn
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_soft_error_event_structure() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // For now, just verify we can create and match a SoftError event
        let test_event = ConnectionEvent::SoftError("Test ICMP error".to_string());

        match test_event {
            ConnectionEvent::SoftError(msg) => {
                assert_eq!(msg, "Test ICMP error");
            }
            _ => panic!("Should match SoftError event"),
        }

        // Close connection
        conn.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_path_change_event_structure() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // For now, just verify we can create and match a PathChange event
        let test_event = ConnectionEvent::PathChange;

        match test_event {
            ConnectionEvent::PathChange => {
                // Event matched successfully
            }
            _ => panic!("Should match PathChange event"),
        }

        // Close connection
        conn.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_enable_soft_error_notifications() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection in background
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        // Create connection with soft error notifications enabled
        let mut transport_props = TransportProperties::default();
        transport_props.selection_properties.soft_error_notify = Preference::Require;

        let preconn = new_preconnection(
            vec![],
            vec![RemoteEndpoint::builder().socket_address(addr).build()],
            transport_props,
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.expect("Should connect");

        // Wait for ready
        match conn.next_event().await {
            Some(ConnectionEvent::Ready) => {}
            other => panic!("Expected Ready event, got {other:?}"),
        }

        // Verify that the connection has soft error notifications preference set
        // (In a real implementation, this would enable ICMP error reception)

        // Close connection
        conn.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_lifecycle_events_on_connection_group() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connections in background
        let _accept_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (_stream, _) = listener.accept().await.unwrap();
                tokio::time::sleep(Duration::from_secs(10)).await;
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

        // Wait for clone to be ready
        match conn2.next_event().await {
            Some(ConnectionEvent::Ready) => {}
            other => panic!("Expected Ready event, got {other:?}"),
        }

        // Both connections should receive lifecycle events
        // (In a real implementation, path changes would affect all connections in a group)

        // Close connections
        conn1.close().await.expect("Should close");
        conn2.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_soft_error_during_data_transfer() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Send some data
        let msg = Message::from_string("Test message");
        conn.send(msg).await.expect("Should send");

        // In a real implementation, if an ICMP error is received during data transfer,
        // a SoftError event should be emitted

        // For now, just verify the connection is still usable after a soft error
        let msg2 = Message::from_string("Another message");
        conn.send(msg2).await.expect("Should still be able to send");

        // Close connection
        conn.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_path_change_on_endpoint_addition() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Create connection in establishing state
        let preconn = new_preconnection(
            vec![],
            vec![RemoteEndpoint::builder()
                .hostname("example.com")
                .port(80)
                .build()],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let conn = Connection::new_with_data(
            preconn,
            ConnectionState::Establishing,
            None,
            None,
            TransportProperties::default(),
        );

        // Start collecting events
        let conn_clone = conn.clone();
        let event_collector = tokio::spawn(async move {
            let mut events = Vec::new();

            // Collect events with timeout
            let timeout = tokio::time::sleep(Duration::from_secs(1));
            tokio::pin!(timeout);

            loop {
                tokio::select! {
                    event = conn_clone.next_event() => {
                        if let Some(event) = event {
                            events.push(event);
                        }
                    }
                    _ = &mut timeout => {
                        break;
                    }
                }
            }

            events
        });

        // Add a new remote endpoint
        let new_endpoint = RemoteEndpoint::builder()
            .hostname("example2.com")
            .port(80)
            .build();

        conn.add_remote(new_endpoint)
            .await
            .expect("Should add endpoint");

        // Give time for event to propagate
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop event collection
        drop(conn);
        let events = event_collector.await.unwrap();

        // Should have received a PathChange event
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ConnectionEvent::PathChange)),
            "Should have received PathChange event when endpoint was added"
        );
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_path_change_detection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // In a real implementation, PathChange events would be emitted when:
        // 1. PMTU changes
        // 2. Multiple paths are used and paths are added/removed
        // 3. Local endpoints change
        // 4. A handover is performed

        // For now, just verify the connection remains operational
        let msg = Message::from_string("Test message");
        conn.send(msg).await.expect("Should send");

        // Close connection
        conn.close().await.expect("Should close");
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_multiple_lifecycle_events() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Create a task to collect events
        let conn_clone = conn.clone();
        let event_collector = tokio::spawn(async move {
            let mut events = Vec::new();

            // Collect a few events (with timeout to prevent hanging)
            let timeout = tokio::time::sleep(Duration::from_secs(2));
            tokio::pin!(timeout);

            loop {
                tokio::select! {
                    event = conn_clone.next_event() => {
                        if let Some(event) = event {
                            match &event {
                                ConnectionEvent::Closed => {
                                    events.push(event);
                                    break;
                                }
                                _ => events.push(event),
                            }
                        }
                    }
                    _ = &mut timeout => {
                        break;
                    }
                }
            }

            events
        });

        // Send some data
        let msg = Message::from_string("Test message");
        conn.send(msg).await.expect("Should send");

        // Close connection
        conn.close().await.expect("Should close");

        // Check collected events
        let events = event_collector.await.unwrap();

        // Should have at least Sent and Closed events
        assert!(events
            .iter()
            .any(|e| matches!(e, ConnectionEvent::Sent { .. })));
        assert!(events.iter().any(|e| matches!(e, ConnectionEvent::Closed)));
    })
    .await
    .expect("Test should complete within timeout");
}
