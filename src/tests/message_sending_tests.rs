//! Unit tests for Message Sending functionality

use crate::{
    message::SendContext, ConnectionEvent, ConnectionState, Message, Preconnection, RemoteEndpoint,
    SecurityParameters, TransportProperties,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;

#[tokio::test]
async fn test_basic_message_send() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Create connection
    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(conn.state().await, ConnectionState::Established);

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Send a message
    let message = Message::from_string("Hello, Transport Services!");
    let result = conn.send(message).await;
    assert!(result.is_ok());

    // Verify we get a Sent event
    let event = conn.next_event().await;
    match event {
        Some(ConnectionEvent::Sent { message_id }) => {
            assert!(message_id.is_some());
        }
        _ => panic!("Expected Sent event, got: {event:?}"),
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_message_with_id() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Send a message with specific ID
    let message = Message::from_string("Test message").with_id(42);
    conn.send(message).await.unwrap();

    // Verify we get a Sent event with correct ID
    let event = conn.next_event().await;
    match event {
        Some(ConnectionEvent::Sent { message_id }) => {
            assert_eq!(message_id, Some(42));
        }
        _ => panic!("Expected Sent event, got: {event:?}"),
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_partial_sends() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Send partial messages
    let msg1 = Message::partial(b"Hello, ".to_vec());
    let msg2 = Message::partial(b"Transport ".to_vec());
    let msg3 = Message::from_bytes(b"Services!"); // end_of_message = true by default

    assert!(!msg1.is_end_of_message());
    assert!(!msg2.is_end_of_message());
    assert!(msg3.is_end_of_message());

    conn.send(msg1).await.unwrap();
    conn.send(msg2).await.unwrap();
    conn.send(msg3).await.unwrap();

    // Verify we get Sent events for all parts
    for _ in 0..3 {
        let event = conn.next_event().await;
        assert!(matches!(event, Some(ConnectionEvent::Sent { .. })));
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_message_batching() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Start batching
    conn.start_batch().await.unwrap();

    // Send multiple messages
    for i in 0..5 {
        let msg = Message::from_string(&format!("Batch message {i}"));
        conn.send(msg).await.unwrap();
    }

    // No Sent events should be received yet (messages are batched)
    // Try to get an event with a short timeout
    let event_result = tokio::time::timeout(Duration::from_millis(50), conn.next_event()).await;
    assert!(event_result.is_err(), "Expected timeout but got event"); // Timeout expected

    // End batching - messages should be sent
    conn.end_batch().await.unwrap();

    // Now we should get all Sent events
    for _ in 0..5 {
        let event = conn.next_event().await;
        assert!(matches!(event, Some(ConnectionEvent::Sent { .. })));
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_message_expiry() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Create an already-expired message
    let context = SendContext {
        expiry: Some(Instant::now() - Duration::from_secs(1)),
        bundle: false,
        completion_notifier: None,
    };

    let message = Message::from_string("This should expire")
        .with_id(99)
        .with_send_context(context);

    // Try to send expired message
    let result = conn.send(message).await;
    assert!(result.is_err());

    // The Expired event should have been sent immediately
    // Try to get it with a timeout
    let event = tokio::time::timeout(Duration::from_millis(100), conn.next_event()).await;
    match event {
        Ok(Some(ConnectionEvent::Expired { message_id })) => {
            assert_eq!(message_id, Some(99));
        }
        Ok(other) => panic!("Expected Expired event, got: {other:?}"),
        Err(_) => panic!("Timeout waiting for Expired event"),
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_initiate_with_send() {
    // Start a test server that echoes received data
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                if let Ok(n) = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await {
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &buf[..n]).await;
                }
            });
        }
    });

    let remote = RemoteEndpoint::builder()
        .socket_address(server_addr)
        .build();

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Initiate with a message
    let message = Message::from_string("Hello from InitiateWithSend!");
    let conn = preconn.initiate_with_send(message).await.unwrap();

    // The message should be queued and sent once established
    // Wait for establishment and send
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Give time for the message to be sent
    sleep(Duration::from_millis(50)).await;

    // Verify we got a Sent event
    let event = conn.next_event().await;
    assert!(matches!(event, Some(ConnectionEvent::Sent { .. })));

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_send_on_closed_connection() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept and immediately close connection
    let accept_handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            drop(stream); // Immediately close
        }
    });

    let preconn = Preconnection::new(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for ready event
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        _ => panic!("Expected Ready event"),
    }

    // Wait for the server to close the connection
    sleep(Duration::from_millis(100)).await;

    // Force the connection to be closed by sending Final
    let final_msg = Message::from_string("Final").with_final(true);
    let _ = conn.send(final_msg).await;

    // Now try to send on closed connection
    let message = Message::from_string("This should fail");
    let result = conn.send(message).await;
    assert!(result.is_err(), "Send should fail on closed connection");

    let _ = accept_handle.await;
}

#[tokio::test]
async fn test_send_with_event_notifier() {
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

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        sleep(Duration::from_millis(10)).await;
    }

    // Consume the Ready event
    let ready_event = conn.next_event().await;
    assert!(matches!(ready_event, Some(ConnectionEvent::Ready)));

    // Create a custom event notifier
    let (tx, _rx) = mpsc::unbounded_channel();
    let context = SendContext {
        expiry: None,
        bundle: false,
        completion_notifier: Some(Arc::new(tx)),
    };

    let message = Message::from_string("Test with notifier")
        .with_id(123)
        .with_send_context(context);

    conn.send(message).await.unwrap();

    // We should get the event through both channels
    let conn_event = conn.next_event().await;
    assert!(matches!(conn_event, Some(ConnectionEvent::Sent { .. })));

    // Note: Custom notifier would need to be implemented in send_message_internal
    // For now, this test shows the structure

    conn.close().await.unwrap();
}
