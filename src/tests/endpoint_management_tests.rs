//! Tests for endpoint management functionality

use crate::*;

#[tokio::test]
async fn test_add_remote_endpoint_to_establishing_connection() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
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

    // Add a new remote endpoint
    let new_endpoint = RemoteEndpoint::builder()
        .ip_address("192.168.1.1".parse().unwrap())
        .port(8080)
        .build();

    conn.add_remote(new_endpoint.clone())
        .await
        .expect("Should be able to add remote endpoint");

    // Verify it was set (for establishing connections without a stream)
    let remote = conn.remote_endpoint().await;
    assert!(remote.is_some());
}

#[tokio::test]
async fn test_add_duplicate_remote_endpoint() {
    let endpoint = RemoteEndpoint::builder()
        .hostname("example.com")
        .port(443)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![endpoint.clone()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Established,
        None,
        Some(endpoint.clone()),
        TransportProperties::default(),
    );

    // Try to add the same endpoint again
    conn.add_remote(endpoint)
        .await
        .expect("Should silently ignore duplicate endpoint");
}

#[tokio::test]
async fn test_add_remote_to_closed_connection() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Closed,
        None,
        None,
        TransportProperties::default(),
    );

    let new_endpoint = RemoteEndpoint::builder()
        .ip_address("192.168.1.1".parse().unwrap())
        .port(8080)
        .build();

    let result = conn.add_remote(new_endpoint).await;
    assert!(result.is_err());

    if let Err(e) = result {
        match e {
            TransportServicesError::InvalidState(msg) => {
                assert!(msg.contains("closed connection"));
            }
            _ => panic!("Wrong error type"),
        }
    }
}

#[tokio::test]
async fn test_add_local_endpoint_to_establishing_connection() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
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

    // Add a new local endpoint
    let new_endpoint = LocalEndpoint {
        identifiers: vec![EndpointIdentifier::Interface("eth0".to_string())],
    };

    conn.add_local(new_endpoint.clone())
        .await
        .expect("Should be able to add local endpoint");

    // Verify it was set (for establishing connections without a stream)
    let local = conn.local_endpoint().await;
    assert!(local.is_some());
}

#[tokio::test]
async fn test_add_duplicate_local_endpoint() {
    let endpoint = LocalEndpoint {
        identifiers: vec![EndpointIdentifier::Interface("eth0".to_string())],
    };

    let preconn = new_preconnection(
        vec![endpoint.clone()],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Established,
        Some(endpoint.clone()),
        None,
        TransportProperties::default(),
    );

    // Try to add the same endpoint again
    conn.add_local(endpoint)
        .await
        .expect("Should silently ignore duplicate endpoint");
}

#[tokio::test]
async fn test_add_local_to_closed_connection() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Closed,
        None,
        None,
        TransportProperties::default(),
    );

    let new_endpoint = LocalEndpoint {
        identifiers: vec![EndpointIdentifier::Interface("eth0".to_string())],
    };

    let result = conn.add_local(new_endpoint).await;
    assert!(result.is_err());

    if let Err(e) = result {
        match e {
            TransportServicesError::InvalidState(msg) => {
                assert!(msg.contains("closed connection"));
            }
            _ => panic!("Wrong error type"),
        }
    }
}
