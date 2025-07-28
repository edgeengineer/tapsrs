use crate::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

/// Start a simple echo server for testing
async fn start_echo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buffer = [0; 1024];
            while let Ok(n) = stream.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
                if stream.write_all(&buffer[..n]).await.is_err() {
                    break;
                }
            }
        }
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

#[tokio::test]
async fn test_connection_establishment() {
    let test_body = async {
        let server_addr = start_echo_server().await;

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let connection = preconn.initiate().await.unwrap();

        // Wait for connection to be established
        let mut established = false;
        for _ in 0..10 {
            match connection.state().await {
                ConnectionState::Established => {
                    established = true;
                    break;
                }
                ConnectionState::Establishing => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                _ => break,
            }
        }

        assert!(established, "Connection should be established");

        // Clean up
        connection.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .expect("Test timed out");
}

#[tokio::test]
async fn test_connection_send_receive() {
    let test_body = async {
        let server_addr = start_echo_server().await;

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let connection = preconn.initiate().await.unwrap();

        // Wait for establishment
        while connection.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Send a message
        let msg = Message::from_string("Hello, Transport Services!");
        connection.send(msg).await.unwrap();

        // Note: Receive is not yet implemented, so we can't test echo
        // But at least verify send doesn't fail

        connection.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .expect("Test timed out");
}

#[tokio::test]
async fn test_connection_events() {
    let test_body = async {
        let server_addr = start_echo_server().await;

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let connection = preconn.initiate().await.unwrap();

        // Listen for Ready event
        let event = timeout(Duration::from_secs(5), connection.next_event()).await;
        assert!(event.is_ok(), "Should receive event within timeout");

        if let Ok(Some(evt)) = event {
            match evt {
                ConnectionEvent::Ready => {
                    // Expected
                }
                ConnectionEvent::EstablishmentError(msg) => {
                    panic!("Unexpected establishment error: {}", msg);
                }
                _ => panic!("Unexpected event type"),
            }
        }

        // Close and wait for Closed event
        connection.close().await.unwrap();

        let event = timeout(Duration::from_secs(1), connection.next_event()).await;
        if let Ok(Some(ConnectionEvent::Closed)) = event {
            // Expected
        } else {
            panic!("Should receive Closed event");
        }
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .expect("Test timed out");
}

#[tokio::test]
async fn test_connection_timeout() {
    // Try to connect to a non-existent address
    let remote = RemoteEndpoint::builder()
        .ip_address("192.0.2.1".parse().unwrap()) // TEST-NET-1, should not be routable
        .port(12345)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let connection = preconn
        .initiate_with_timeout(Some(Duration::from_secs(1)))
        .await
        .unwrap();

    // Wait for establishment error
    let event = timeout(Duration::from_secs(2), connection.next_event()).await;

    if let Ok(Some(evt)) = event {
        match evt {
            ConnectionEvent::EstablishmentError(_) => {
                // Expected
            }
            _ => panic!("Expected establishment error"),
        }
    } else {
        panic!("Should receive establishment error event");
    }

    assert_eq!(connection.state().await, ConnectionState::Closed);
}

#[tokio::test]
async fn test_message_properties() {
    let msg = Message::from_string("Test message")
        .with_lifetime(Duration::from_secs(60))
        .with_priority(100)
        .safely_replayable()
        .final_message();

    assert_eq!(msg.properties().lifetime, Some(Duration::from_secs(60)));
    assert_eq!(msg.properties().priority, Some(100));
    assert!(msg.properties().safely_replayable);
    assert!(msg.properties().final_message);
}

#[tokio::test]
async fn test_queued_messages() {
    let test_body = async {
        let server_addr = start_echo_server().await;

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let connection = preconn.initiate().await.unwrap();

        // Send messages while still establishing
        let msg1 = Message::from_string("Message 1");
        let msg2 = Message::from_string("Message 2");

        // These should be queued
        connection.send(msg1).await.unwrap();
        connection.send(msg2).await.unwrap();

        // Wait for establishment
        while connection.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Messages should have been sent automatically
        // (We can't verify this without receive, but at least no errors)

        connection.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .expect("Test timed out");
}
