//! Tests for read-only generic connection properties (RFC 8.1.11)

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
        tokio::time::sleep(Duration::from_secs(1)).await;
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
async fn test_conn_state_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that connection state is Established
        if let Some(ConnectionProperty::ConnState(state)) = conn.get_property("connState").await {
            assert_eq!(state, ConnectionState::Established);
        } else {
            panic!("connState property not found");
        }

        // Close the connection
        conn.close().await.expect("Should close");

        // Check that connection state is now Closed
        if let Some(ConnectionProperty::ConnState(state)) = conn.get_property("connState").await {
            assert_eq!(state, ConnectionState::Closed);
        } else {
            panic!("connState property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_can_send_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that we can send on an established connection
        if let Some(ConnectionProperty::CanSend(can_send)) = conn.get_property("canSend").await {
            assert!(can_send, "Should be able to send on established connection");
        } else {
            panic!("canSend property not found");
        }

        // Send a Final message
        let final_msg = Message::from_string("Final message").with_final(true);
        conn.send(final_msg)
            .await
            .expect("Should send final message");

        // Check that we can no longer send
        if let Some(ConnectionProperty::CanSend(can_send)) = conn.get_property("canSend").await {
            assert!(!can_send, "Should not be able to send after Final message");
        } else {
            panic!("canSend property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_can_receive_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that we can receive on an established connection
        if let Some(ConnectionProperty::CanReceive(can_receive)) =
            conn.get_property("canReceive").await
        {
            assert!(
                can_receive,
                "Should be able to receive on established connection"
            );
        } else {
            panic!("canReceive property not found");
        }

        // Close the connection
        conn.close().await.expect("Should close");

        // Check that we can no longer receive
        if let Some(ConnectionProperty::CanReceive(can_receive)) =
            conn.get_property("canReceive").await
        {
            assert!(
                !can_receive,
                "Should not be able to receive on closed connection"
            );
        } else {
            panic!("canReceive property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unidirectional_send_connection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection in background
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        // Create unidirectional send connection
        let transport_props = TransportProperties::builder()
            .direction(CommunicationDirection::UnidirectionalSend)
            .build();

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

        // Check that we can send but not receive
        if let Some(ConnectionProperty::CanSend(can_send)) = conn.get_property("canSend").await {
            assert!(
                can_send,
                "Should be able to send on unidirectional send connection"
            );
        } else {
            panic!("canSend property not found");
        }

        if let Some(ConnectionProperty::CanReceive(can_receive)) =
            conn.get_property("canReceive").await
        {
            assert!(
                !can_receive,
                "Should not be able to receive on unidirectional send connection"
            );
        } else {
            panic!("canReceive property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unidirectional_receive_connection() {
    tokio::time::timeout(Duration::from_secs(5), async {
        // Start a TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept connection in background
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        // Create unidirectional receive connection
        let transport_props = TransportProperties::builder()
            .direction(CommunicationDirection::UnidirectionalReceive)
            .build();

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

        // Check that we can receive but not send
        if let Some(ConnectionProperty::CanSend(can_send)) = conn.get_property("canSend").await {
            assert!(
                !can_send,
                "Should not be able to send on unidirectional receive connection"
            );
        } else {
            panic!("canSend property not found");
        }

        if let Some(ConnectionProperty::CanReceive(can_receive)) =
            conn.get_property("canReceive").await
        {
            assert!(
                can_receive,
                "Should be able to receive on unidirectional receive connection"
            );
        } else {
            panic!("canReceive property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_singular_transmission_msg_max_len() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that singularTransmissionMsgMaxLen is set
        if let Some(ConnectionProperty::SingularTransmissionMsgMaxLen(max_len)) =
            conn.get_property("singularTransmissionMsgMaxLen").await
        {
            assert!(max_len.is_some(), "Should have a value for TCP");
            if let Some(len) = max_len {
                // Typical MSS values range from 536 to 65535
                assert!(len >= 536, "MSS should be at least 536 bytes");
                assert!(len <= 65535, "MSS should not exceed 65535 bytes");
            }
        } else {
            panic!("singularTransmissionMsgMaxLen property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_send_msg_max_len() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that sendMsgMaxLen is None (no limit) for TCP
        if let Some(ConnectionProperty::SendMsgMaxLen(max_len)) =
            conn.get_property("sendMsgMaxLen").await
        {
            assert!(
                max_len.is_none(),
                "TCP should have no send message size limit"
            );
        } else {
            panic!("sendMsgMaxLen property not found");
        }

        // Send a Final message
        let final_msg = Message::from_string("Final message").with_final(true);
        conn.send(final_msg)
            .await
            .expect("Should send final message");

        // Check that sendMsgMaxLen is now 0 (cannot send)
        if let Some(ConnectionProperty::SendMsgMaxLen(max_len)) =
            conn.get_property("sendMsgMaxLen").await
        {
            assert_eq!(
                max_len,
                Some(0),
                "Should return 0 when sending is not possible"
            );
        } else {
            panic!("sendMsgMaxLen property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recv_msg_max_len() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Check that recvMsgMaxLen is None (no limit) for TCP
        if let Some(ConnectionProperty::RecvMsgMaxLen(max_len)) =
            conn.get_property("recvMsgMaxLen").await
        {
            assert!(
                max_len.is_none(),
                "TCP should have no receive message size limit"
            );
        } else {
            panic!("recvMsgMaxLen property not found");
        }

        // Close the connection
        conn.close().await.expect("Should close");

        // Check that recvMsgMaxLen is now 0 (cannot receive)
        if let Some(ConnectionProperty::RecvMsgMaxLen(max_len)) =
            conn.get_property("recvMsgMaxLen").await
        {
            assert_eq!(
                max_len,
                Some(0),
                "Should return 0 when receiving is not possible"
            );
        } else {
            panic!("recvMsgMaxLen property not found");
        }
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_all_readonly_properties_exist() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Get all properties
        let props = conn.get_properties().await;

        // Check that all read-only properties exist
        assert!(props.get("connState").is_some(), "connState should exist");
        assert!(props.get("canSend").is_some(), "canSend should exist");
        assert!(props.get("canReceive").is_some(), "canReceive should exist");
        assert!(
            props.get("singularTransmissionMsgMaxLen").is_some(),
            "singularTransmissionMsgMaxLen should exist"
        );
        assert!(
            props.get("sendMsgMaxLen").is_some(),
            "sendMsgMaxLen should exist"
        );
        assert!(
            props.get("recvMsgMaxLen").is_some(),
            "recvMsgMaxLen should exist"
        );
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_readonly_properties_cannot_be_set() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;

        // Try to set read-only properties - should fail
        let readonly_props = vec![
            (
                "connState",
                ConnectionProperty::ConnState(ConnectionState::Closed),
            ),
            ("canSend", ConnectionProperty::CanSend(false)),
            ("canReceive", ConnectionProperty::CanReceive(false)),
            (
                "singularTransmissionMsgMaxLen",
                ConnectionProperty::SingularTransmissionMsgMaxLen(Some(1000)),
            ),
            (
                "sendMsgMaxLen",
                ConnectionProperty::SendMsgMaxLen(Some(2000)),
            ),
            (
                "recvMsgMaxLen",
                ConnectionProperty::RecvMsgMaxLen(Some(3000)),
            ),
        ];

        for (key, value) in readonly_props {
            let result = conn.set_property(key, value).await;
            assert!(
                result.is_err(),
                "Should not be able to set read-only property: {key}"
            );
            if let Err(e) = result {
                match e {
                    TransportServicesError::InvalidParameters(msg) => {
                        assert!(
                            msg.contains("read-only"),
                            "Error message should indicate property is read-only"
                        );
                    }
                    _ => panic!("Expected InvalidParameters error for read-only property"),
                }
            }
        }
    })
    .await
    .expect("Test should complete within timeout");
}
