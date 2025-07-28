//! Unit tests for Rendezvous functionality

use crate::{
    preconnection::new_preconnection, ConnectionState, EndpointIdentifier, LocalEndpoint,
    RemoteEndpoint, SecurityParameters, TransportProperties,
};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn test_rendezvous_requires_endpoints() {
    // Test with no local endpoints
    let preconn = new_preconnection(
        vec![], // No local endpoints
        vec![RemoteEndpoint {
            identifiers: vec![EndpointIdentifier::Port(8080)],
            protocol: None,
        }],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.rendezvous().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No local endpoints"));

    // Test with no remote endpoints
    let preconn = new_preconnection(
        vec![LocalEndpoint {
            identifiers: vec![EndpointIdentifier::Port(8080)],
        }],
        vec![], // No remote endpoints
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.rendezvous().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No remote endpoints"));
}

#[tokio::test]
async fn test_rendezvous_basic() {
    let local = LocalEndpoint {
        identifiers: vec![
            EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
            EndpointIdentifier::Port(0), // Let OS choose
        ],
    };

    let remote = RemoteEndpoint {
        identifiers: vec![
            EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
            EndpointIdentifier::Port(54323), // Non-listening port
        ],
        protocol: None,
    };

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.rendezvous().await;
    assert!(result.is_ok());

    let (connection, listener) = result.unwrap();

    // Verify listener is active
    assert!(listener.is_active().await);

    // Verify connection is in establishing state
    assert_eq!(connection.state().await, ConnectionState::Establishing);

    // Clean up
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_resolve() {
    // Test that resolve() is called during rendezvous
    let local = LocalEndpoint {
        identifiers: vec![EndpointIdentifier::Port(0)], // Should resolve to 0.0.0.0 and ::
    };

    let remote = RemoteEndpoint {
        identifiers: vec![
            EndpointIdentifier::HostName("localhost".to_string()),
            EndpointIdentifier::Port(54324),
        ],
        protocol: None,
    };

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // First test resolve directly
    let (locals, remotes) = preconn.resolve().await.unwrap();
    assert!(locals.len() >= 2); // Should have IPv4 and IPv6
    assert!(remotes.len() >= 1); // Should have resolved localhost

    // Now test rendezvous
    let result = preconn.rendezvous().await;
    assert!(result.is_ok());

    let (_, listener) = result.unwrap();

    // Verify listener bound to an address
    let addr = listener.local_addr().await;
    assert!(addr.is_some());

    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_simultaneous_connect() {
    use tokio::net::TcpListener;

    // Start a peer listener
    let peer_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = peer_listener.local_addr().unwrap();

    // Set up rendezvous endpoints
    let local = LocalEndpoint {
        identifiers: vec![
            EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
            EndpointIdentifier::Port(0),
        ],
    };

    let remote = RemoteEndpoint {
        identifiers: vec![EndpointIdentifier::SocketAddress(peer_addr)],
        protocol: None,
    };

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    // Start rendezvous
    let (connection, listener) = preconn.rendezvous().await.unwrap();

    // Accept peer connection
    let peer_handle = tokio::spawn(async move {
        let (stream, _) = peer_listener.accept().await.unwrap();
        stream
    });

    // Wait for connection to establish (with timeout)
    let mut attempts = 0;
    while connection.state().await == ConnectionState::Establishing && attempts < 10 {
        sleep(Duration::from_millis(50)).await;
        attempts += 1;
    }

    // Verify connection established
    assert_eq!(connection.state().await, ConnectionState::Established);

    // Clean up
    let _ = peer_handle.await;
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_incoming_connection() {
    use tokio::net::TcpStream;

    // Set up rendezvous
    let local = LocalEndpoint {
        identifiers: vec![
            EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
            EndpointIdentifier::Port(0),
        ],
    };

    let remote = RemoteEndpoint {
        identifiers: vec![
            EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
            EndpointIdentifier::Port(65535), // Invalid port - connection will fail
        ],
        protocol: None,
    };

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let (connection, mut listener) = preconn.rendezvous().await.unwrap();
    let listen_addr = listener.local_addr().await.unwrap();

    // Connect to the listener
    let client_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        TcpStream::connect(listen_addr).await.unwrap()
    });

    // Accept the incoming connection
    let incoming_conn = timeout(Duration::from_millis(100), listener.accept())
        .await
        .unwrap()
        .unwrap();

    // Verify we got a connection
    assert_eq!(incoming_conn.state().await, ConnectionState::Established);

    // The original connection should still be establishing (outgoing failed)
    assert_eq!(connection.state().await, ConnectionState::Establishing);

    // Clean up
    let _ = client_handle.await;
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_multiple_endpoints() {
    // Test with multiple local and remote endpoints
    let locals = vec![
        LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        },
        LocalEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("::1".parse().unwrap()),
                EndpointIdentifier::Port(0),
            ],
        },
    ];

    let remotes = vec![
        RemoteEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("127.0.0.1".parse().unwrap()),
                EndpointIdentifier::Port(54325),
            ],
            protocol: None,
        },
        RemoteEndpoint {
            identifiers: vec![
                EndpointIdentifier::IpAddress("::1".parse().unwrap()),
                EndpointIdentifier::Port(54326),
            ],
            protocol: None,
        },
    ];

    let preconn = new_preconnection(
        locals,
        remotes,
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.rendezvous().await;
    assert!(result.is_ok());

    let (_, listener) = result.unwrap();
    listener.stop().await.unwrap();
}
