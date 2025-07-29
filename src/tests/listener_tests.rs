//! Unit tests for Listener implementation

use crate::{
    listener::ListenerEvent, preconnection::new_preconnection, EndpointIdentifier, LocalEndpoint,
    SecurityParameters, TransportProperties,
};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn test_listener_creation() {
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::Port(0), // Let OS choose port
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.listen().await;
    assert!(result.is_ok());
    let listener = result.unwrap();

    // Check that listener is active
    assert!(listener.is_active().await);

    // Stop the listener
    assert!(listener.stop().await.is_ok());
    assert!(!listener.is_active().await);
}

#[tokio::test]
async fn test_listener_with_specific_port() {
    let port = 54321;
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(port),
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let listener = preconn.listen().await.unwrap();

    // Check bound address
    let local_addr = listener.local_addr().await;
    assert!(local_addr.is_some());
    let addr = local_addr.unwrap();
    assert_eq!(addr.port(), port);

    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_listener_no_endpoints_error() {
    let preconn = new_preconnection(
        vec![], // No local endpoints
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.listen().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No local endpoints"));
}

#[tokio::test]
async fn test_listener_accept_with_connection() {
    use tokio::net::TcpStream;

    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let bound_addr = listener.local_addr().await.unwrap();

    // Spawn a client connection with short timeout
    let client_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await; // Brief delay
        TcpStream::connect(bound_addr).await
    });

    // Accept connection with 100ms timeout
    let accept_result = timeout(Duration::from_millis(100), listener.accept()).await;

    assert!(accept_result.is_ok());
    let conn_result = accept_result.unwrap();
    assert!(conn_result.is_ok());

    let connection = conn_result.unwrap();
    assert_eq!(
        connection.state().await,
        crate::ConnectionState::Established
    );

    // Verify endpoints
    let local_ep = connection.local_endpoint().await;
    assert!(local_ep.is_some());

    let remote_ep = connection.remote_endpoint().await;
    assert!(remote_ep.is_some());

    // Clean up
    let _ = client_handle.await;
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_listener_stop() {
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![EndpointIdentifier::Port(0)],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let listener = preconn.listen().await.unwrap();

    // Verify it's active
    assert!(listener.is_active().await);

    // Stop the listener
    listener.stop().await.unwrap();

    // Verify it's not active
    assert!(!listener.is_active().await);
}

#[tokio::test]
async fn test_listener_connection_limit() {
    use tokio::net::TcpStream;

    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let bound_addr = listener.local_addr().await.unwrap();

    // Set connection limit to 1
    listener.set_new_connection_limit(1);

    // Create two client connections
    let client1 = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        TcpStream::connect(bound_addr).await
    });

    let client2 = tokio::spawn(async move {
        sleep(Duration::from_millis(20)).await;
        TcpStream::connect(bound_addr).await
    });

    // First connection should succeed
    let accept1 = timeout(Duration::from_millis(100), listener.accept()).await;
    assert!(accept1.is_ok() && accept1.unwrap().is_ok());

    // Second connection should not be received (limit reached)
    let accept2 = timeout(Duration::from_millis(50), listener.accept()).await;
    assert!(accept2.is_err()); // Should timeout

    // Clean up
    let _ = client1.await;
    let _ = client2.await;
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_listener_event_stream() {
    use tokio::net::TcpStream;

    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let bound_addr = listener.local_addr().await.unwrap();

    // Connect and check event
    tokio::spawn(async move {
        let _ = TcpStream::connect(bound_addr).await;
    });

    let event = timeout(Duration::from_secs(2), listener.next_event()).await;
    assert!(event.is_ok(), "Failed to receive event within timeout");

    if let Some(ListenerEvent::ConnectionReceived(conn)) = event.unwrap() {
        assert_eq!(conn.state().await, crate::ConnectionState::Established);
    } else {
        panic!("Expected ConnectionReceived event");
    }

    // Stop and check stopped event
    listener.stop().await.unwrap();

    let stop_event = timeout(Duration::from_secs(1), listener.next_event()).await;
    assert!(stop_event.is_ok());

    if let Some(ListenerEvent::Stopped) = stop_event.unwrap() {
        // Success
    } else {
        panic!("Expected Stopped event");
    }
}

#[tokio::test]
async fn test_listener_multiple_connections() {
    use tokio::net::TcpStream;

    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let bound_addr = listener.local_addr().await.unwrap();

    // Spawn multiple clients
    let mut client_handles = vec![];
    for i in 0..3 {
        let addr = bound_addr;
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(10 * (i + 1) as u64)).await;
            let stream = TcpStream::connect(addr).await.unwrap();
            // Keep the connection alive
            sleep(Duration::from_secs(1)).await;
            drop(stream);
        });
        client_handles.push(handle);
    }

    // Accept all connections
    let mut connections = vec![];
    for i in 0..3 {
        match timeout(Duration::from_millis(500), listener.accept()).await {
            Ok(Ok(conn)) => connections.push(conn),
            _ => panic!("Failed to accept connection {}", i),
        }
    }

    assert_eq!(connections.len(), 3);

    // Verify all connections are established
    for conn in connections {
        assert_eq!(conn.state().await, crate::ConnectionState::Established);
    }

    listener.stop().await.unwrap();

    // Wait for client tasks to complete
    for handle in client_handles {
        let _ = handle.await;
    }
}

#[tokio::test]
async fn test_listener_socket_address_endpoint() {
    use std::net::SocketAddr;

    let socket_addr: SocketAddr = "127.0.0.1:54322".parse().unwrap();
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![EndpointIdentifier::SocketAddress(socket_addr)],
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let listener = preconn.listen().await.unwrap();

    let local_addr = listener.local_addr().await;
    assert!(local_addr.is_some());
    assert_eq!(local_addr.unwrap(), socket_addr);

    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_listener_bind_any_address() {
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![], // No specific address - should bind to 0.0.0.0:0
        }],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let listener = preconn.listen().await.unwrap();

    let local_addr = listener.local_addr().await;
    assert!(local_addr.is_some());
    let addr = local_addr.unwrap();
    assert!(addr.port() > 0); // OS should assign a port

    listener.stop().await.unwrap();
}
