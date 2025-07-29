//! Tests for Connection Termination (RFC Section 10)

use crate::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn create_test_connection() -> Connection {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection in background
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // Echo server for testing
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

    // Create connection
    let preconn = Preconnection::new(
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
async fn test_close_graceful_termination() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Send some data before closing
        let msg = Message::from_string("Hello, server!");
        conn.send(msg).await.expect("Should send");

        // Wait for sent event
        match conn.next_event().await {
            Some(ConnectionEvent::Sent { .. }) => {}
            other => panic!("Expected Sent event, got {other:?}"),
        }

        // Close gracefully
        conn.close().await.expect("Should close");

        // Should receive Closed event
        match conn.next_event().await {
            Some(ConnectionEvent::Closed) => {}
            other => panic!("Expected Closed event, got {other:?}"),
        }

        // Verify connection state
        assert_eq!(conn.state().await, ConnectionState::Closed);

        // Verify we can't send after close
        let msg2 = Message::from_string("Should fail");
        assert!(conn.send(msg2).await.is_err());
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_abort_immediate_termination() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Add some pending messages
        conn.start_batch().await.unwrap();
        conn.send(Message::from_string("Pending 1")).await.unwrap();
        conn.send(Message::from_string("Pending 2")).await.unwrap();

        // Abort immediately
        conn.abort().await.expect("Should abort");

        // Should receive ConnectionError event
        match conn.next_event().await {
            Some(ConnectionEvent::ConnectionError(msg)) => {
                assert!(msg.contains("aborted"));
            }
            other => panic!("Expected ConnectionError event, got {other:?}"),
        }

        // Verify connection state
        assert_eq!(conn.state().await, ConnectionState::Closed);

        // Verify we can't send after abort
        let msg = Message::from_string("Should fail");
        assert!(conn.send(msg).await.is_err());
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_close_delivers_pending_messages() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Send multiple messages
        for i in 0..5 {
            let msg = Message::from_string(&format!("Message {i}"));
            conn.send(msg).await.expect("Should send");
        }

        // Close gracefully - should deliver all messages
        conn.close().await.expect("Should close");

        // Should receive Closed event after all messages are delivered
        let mut sent_count = 0;
        loop {
            match conn.next_event().await {
                Some(ConnectionEvent::Sent { .. }) => {
                    sent_count += 1;
                }
                Some(ConnectionEvent::Closed) => {
                    break;
                }
                other => panic!("Unexpected event: {other:?}"),
            }
        }

        // Should have sent all messages before closing
        assert_eq!(sent_count, 5);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_abort_discards_pending_messages() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Start a batch and add messages
        conn.start_batch().await.unwrap();
        for i in 0..5 {
            let msg = Message::from_string(&format!("Pending {i}"));
            conn.send(msg).await.expect("Should add to batch");
        }

        // Abort without ending batch - messages should be discarded
        conn.abort().await.expect("Should abort");

        // Should receive ConnectionError immediately, no Sent events
        match conn.next_event().await {
            Some(ConnectionEvent::ConnectionError(msg)) => {
                assert!(msg.contains("aborted"));
            }
            other => panic!("Expected ConnectionError event, got {other:?}"),
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_close_on_already_closed_connection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Close once
        conn.close().await.expect("Should close");

        // Wait for Closed event
        match conn.next_event().await {
            Some(ConnectionEvent::Closed) => {}
            other => panic!("Expected Closed event, got {other:?}"),
        }

        // Close again - should be no-op
        conn.close().await.expect("Should be no-op");

        // State should still be Closed
        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_abort_on_already_closed_connection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Close gracefully first
        conn.close().await.expect("Should close");

        // Wait for Closed event
        match conn.next_event().await {
            Some(ConnectionEvent::Closed) => {}
            other => panic!("Expected Closed event, got {other:?}"),
        }

        // Abort after close - should be no-op
        conn.abort().await.expect("Should be no-op");

        // State should still be Closed
        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_remote_close_detection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection and close from server side
        let server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Send some data
            stream.write_all(b"Hello").await.unwrap();
            stream.flush().await.unwrap();
            // Wait a bit then close
            tokio::time::sleep(Duration::from_millis(200)).await;
            // Just drop the stream to close the connection
            drop(stream);
        });

        // Create connection
        let preconn = Preconnection::new(
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

        // Wait for events from background reader
        let mut received_hello = false;
        let mut received_closed = false;

        let start = std::time::Instant::now();
        while (!received_hello || !received_closed) && start.elapsed() < Duration::from_secs(3) {
            match tokio::time::timeout(Duration::from_millis(500), conn.next_event()).await {
                Ok(Some(ConnectionEvent::Received { message_data, .. })) => {
                    assert_eq!(message_data, b"Hello");
                    received_hello = true;
                }
                Ok(Some(ConnectionEvent::Closed)) => {
                    received_closed = true;
                }
                Ok(Some(_)) => {} // Ignore other events
                Ok(None) => break,
                Err(_) => {} // Timeout, continue
            }
        }

        // Wait for server to close
        server_task.await.unwrap();

        // Ensure we received the message and detected the close
        assert!(received_hello, "Should have received Hello message");
        assert!(received_closed, "Should have detected remote close");

        // The connection should eventually realize it's closed
        // (though this may require additional implementation)
        tokio::time::sleep(Duration::from_millis(100)).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_connection_state_transitions_during_close() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Should be Established
        assert_eq!(conn.state().await, ConnectionState::Established);

        // Start closing
        let close_handle = tokio::spawn({
            let conn = conn.clone();
            async move { conn.close().await }
        });

        // Give it a moment to transition to Closing
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Check state during close (might be Closing or already Closed)
        let state = conn.state().await;
        assert!(matches!(
            state,
            ConnectionState::Closing | ConnectionState::Closed
        ));

        // Wait for close to complete
        close_handle.await.unwrap().expect("Should close");

        // Should be Closed
        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_connection_state_transitions_during_abort() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Should be Established
        assert_eq!(conn.state().await, ConnectionState::Established);

        // Abort immediately transitions to Closed
        conn.abort().await.expect("Should abort");

        // Should be Closed immediately
        assert_eq!(conn.state().await, ConnectionState::Closed);
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_termination_events_are_terminal() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Close the connection
        conn.close().await.expect("Should close");

        // Get the Closed event
        match conn.next_event().await {
            Some(ConnectionEvent::Closed) => {}
            other => panic!("Expected Closed event, got {other:?}"),
        }

        // No more events should be generated
        let timeout = tokio::time::sleep(Duration::from_millis(100));
        tokio::pin!(timeout);

        tokio::select! {
            event = conn.next_event() => {
                panic!("Should not receive any more events after Closed, got {event:?}");
            }
            _ = timeout => {
                // Expected - no more events
            }
        }
    })
    .await
    .expect("Test should complete within timeout");
}
