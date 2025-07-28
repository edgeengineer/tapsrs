//! Tests for MTU/MSS functionality

use crate::*;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_tcp_mss_query() {
    // Start a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept connections in background
    let accept_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        // Keep the connection alive
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        drop(stream);
    });

    // Create a connection
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder().socket_address(addr).build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.expect("Should connect");

    // Wait for connection to be established
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {}
        other => panic!("Expected Ready event, got {other:?}"),
    }

    // Get properties
    let props = conn.get_properties().await;

    // Check that we have MSS property
    if let Some(ConnectionProperty::SingularTransmissionMsgMaxLen(Some(mss))) =
        props.get("singularTransmissionMsgMaxLen")
    {
        // MSS should be reasonable
        // Note: Loopback interfaces often have very large MSS (16K+)
        // Regular networks typically have MSS between 500-9000
        assert!(*mss >= 500, "MSS too small: {mss}");
        assert!(*mss <= 65535, "MSS exceeds maximum possible value: {mss}");
        println!("TCP MSS: {mss} bytes");

        // Check if this looks like a loopback MSS
        if *mss > 9000 {
            println!("Note: Large MSS detected, likely loopback interface");
        }
    } else {
        panic!("singularTransmissionMsgMaxLen property not found or None");
    }

    // Check that sendMsgMaxLen and recvMsgMaxLen are None (no limit for TCP)
    if let Some(ConnectionProperty::SendMsgMaxLen(limit)) = props.get("sendMsgMaxLen") {
        assert_eq!(*limit, None, "TCP should have no send message size limit");
    } else {
        panic!("sendMsgMaxLen property not found");
    }

    if let Some(ConnectionProperty::RecvMsgMaxLen(limit)) = props.get("recvMsgMaxLen") {
        assert_eq!(
            *limit, None,
            "TCP should have no receive message size limit"
        );
    } else {
        panic!("recvMsgMaxLen property not found");
    }

    conn.close().await.expect("Should close");
    accept_task.await.unwrap();
}

#[tokio::test]
async fn test_mss_property_not_set_before_connection() {
    let preconn = new_preconnection(
        vec![],
        vec![RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    // Create a connection in Establishing state (not connected)
    let conn = Connection::new_with_data(
        preconn,
        ConnectionState::Establishing,
        None,
        None,
        TransportProperties::default(),
    );

    // Get properties
    let props = conn.get_properties().await;

    // Should not have MSS property when not connected
    assert!(!props.has("singularTransmissionMsgMaxLen"));
    assert!(!props.has("sendMsgMaxLen"));
    assert!(!props.has("recvMsgMaxLen"));
}
