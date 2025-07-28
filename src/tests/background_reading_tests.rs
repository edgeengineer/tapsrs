//! Tests for background reading functionality

use crate::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_background_reading_receives_messages() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection and send data in background
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send messages with delay
        sleep(Duration::from_millis(100)).await;
        stream.write_all(b"Hello").await.unwrap();
        stream.flush().await.unwrap();

        sleep(Duration::from_millis(100)).await;
        stream.write_all(b"World").await.unwrap();
        stream.flush().await.unwrap();

        // Keep connection alive
        sleep(Duration::from_secs(1)).await;
    });

    // Create connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.expect("Should connect");

    // Wait for ready event
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Collect received events
    let mut received_messages = Vec::new();

    // Wait for messages via events
    let start = std::time::Instant::now();
    while received_messages.len() < 2 && start.elapsed() < Duration::from_secs(2) {
        match tokio::time::timeout(Duration::from_millis(500), conn.next_event()).await {
            Ok(Some(ConnectionEvent::Received { message_data, .. })) => {
                received_messages.push(String::from_utf8(message_data).unwrap());
            }
            Ok(Some(_)) => {} // Ignore other events
            Ok(None) => break,
            Err(_) => {} // Timeout, continue
        }
    }

    // Verify we received both messages
    assert_eq!(received_messages.len(), 2);
    assert_eq!(received_messages[0], "Hello");
    assert_eq!(received_messages[1], "World");

    conn.close().await.unwrap();
    let _ = server_task.await;
}

#[tokio::test]
async fn test_background_reading_with_framing() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection and send framed data in background
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send length-prefixed messages
        sleep(Duration::from_millis(100)).await;

        // First message: "Hello" (5 bytes)
        let msg1 = b"Hello";
        let len1 = (msg1.len() as u32).to_be_bytes();
        stream.write_all(&len1).await.unwrap();
        stream.write_all(msg1).await.unwrap();
        stream.flush().await.unwrap();

        sleep(Duration::from_millis(100)).await;

        // Second message: "World!" (6 bytes)
        let msg2 = b"World!";
        let len2 = (msg2.len() as u32).to_be_bytes();
        stream.write_all(&len2).await.unwrap();
        stream.write_all(msg2).await.unwrap();
        stream.flush().await.unwrap();

        // Keep connection alive
        sleep(Duration::from_secs(1)).await;
    });

    // Create connection with framing
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.expect("Should connect");

    // Wait for ready event
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Enable length-prefix framing
    conn.use_length_prefix_framer().await.unwrap();

    // Collect received events
    let mut received_messages = Vec::new();

    // Wait for messages via events
    let start = std::time::Instant::now();
    while received_messages.len() < 2 && start.elapsed() < Duration::from_secs(2) {
        match tokio::time::timeout(Duration::from_millis(500), conn.next_event()).await {
            Ok(Some(ConnectionEvent::Received { message_data, .. })) => {
                received_messages.push(String::from_utf8(message_data).unwrap());
            }
            Ok(Some(_)) => {} // Ignore other events
            Ok(None) => break,
            Err(_) => {} // Timeout, continue
        }
    }

    // Verify we received both messages correctly framed
    assert_eq!(received_messages.len(), 2);
    assert_eq!(received_messages[0], "Hello");
    assert_eq!(received_messages[1], "World!");

    conn.close().await.unwrap();
    let _ = server_task.await;
}

#[tokio::test]
async fn test_background_reading_handles_connection_close() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection and close after sending data
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send a message then close
        stream.write_all(b"Goodbye").await.unwrap();
        stream.flush().await.unwrap();
        sleep(Duration::from_millis(100)).await;
        drop(stream); // Close connection
    });

    // Create connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.expect("Should connect");

    // Wait for ready event
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Collect events
    let mut received_message = false;
    let mut connection_closed = false;

    // Wait for events
    let start = std::time::Instant::now();
    while (!received_message || !connection_closed) && start.elapsed() < Duration::from_secs(2) {
        match tokio::time::timeout(Duration::from_millis(500), conn.next_event()).await {
            Ok(Some(ConnectionEvent::Received { message_data, .. })) => {
                assert_eq!(String::from_utf8(message_data).unwrap(), "Goodbye");
                received_message = true;
            }
            Ok(Some(ConnectionEvent::Closed)) => {
                connection_closed = true;
            }
            Ok(Some(_)) => {} // Ignore other events
            Ok(None) => break,
            Err(_) => {} // Timeout, continue
        }
    }

    // Verify we received the message and closed event
    assert!(received_message, "Should have received message");
    assert!(connection_closed, "Should have received closed event");

    // Connection state should be closed
    assert_eq!(conn.state().await, ConnectionState::Closed);

    let _ = server_task.await;
}

#[tokio::test]
async fn test_background_reading_concurrent_with_receive() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connection and send data in background
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send two messages with delays
        sleep(Duration::from_millis(200)).await;
        stream.write_all(b"Event1").await.unwrap();
        stream.flush().await.unwrap();

        sleep(Duration::from_millis(200)).await;
        stream.write_all(b"Event2").await.unwrap();
        stream.flush().await.unwrap();

        // Keep alive to receive manual message
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf).await;
    });

    // Create connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.expect("Should connect");

    // Wait for ready event
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Clone connection for concurrent operations
    let conn_clone = conn.clone();

    // Start a task to collect events
    let event_task = tokio::spawn(async move {
        let mut events = Vec::new();
        let start = std::time::Instant::now();

        while events.len() < 2 && start.elapsed() < Duration::from_secs(2) {
            match tokio::time::timeout(Duration::from_millis(500), conn_clone.next_event()).await {
                Ok(Some(ConnectionEvent::Received { message_data, .. })) => {
                    events.push(String::from_utf8(message_data).unwrap());
                }
                Ok(Some(_)) => {} // Ignore other events
                Ok(None) => break,
                Err(_) => {} // Timeout, continue
            }
        }

        events
    });

    // Meanwhile, also test that manual receive() still works
    sleep(Duration::from_millis(100)).await;

    // Send a message
    conn.send(Message::from_bytes(b"Manual")).await.unwrap();

    // Wait for event collection
    let events = event_task.await.unwrap();

    // Should have received events via background reading
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], "Event1");
    assert_eq!(events[1], "Event2");

    conn.close().await.unwrap();
    let _ = server_task.await;
}
