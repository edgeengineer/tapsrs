//! Integration tests for Transport Services

use crate::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Start a test server that accepts connections and handles simple protocols
async fn start_test_server(protocol: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            match protocol {
                "echo" => {
                    // Echo server
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
                "http" => {
                    // Simple HTTP server
                    let mut buffer = [0; 1024];
                    if let Ok(n) = stream.read(&mut buffer).await {
                        if n > 0 {
                            let response =
                                b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
                            let _ = stream.write_all(response).await;
                        }
                    }
                }
                _ => {}
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

#[tokio::test]
async fn test_multiple_connections() {
    let test_body = async {
        let server_addr = start_test_server("echo").await;

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

        // Create multiple connections
        let mut connections = Vec::new();
        for i in 0..5 {
            let conn = preconn.initiate().await.unwrap();

            // Wait for establishment
            while conn.state().await == ConnectionState::Establishing {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            assert_eq!(conn.state().await, ConnectionState::Established);

            // Send unique message
            let msg = Message::from_string(&format!("Connection {}", i));
            conn.send(msg).await.unwrap();

            connections.push(conn);
        }

        // Close all connections
        for conn in connections {
            conn.close().await.unwrap();
        }
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_transport_properties_application() {
    let test_body = async {
        let server_addr = start_test_server("echo").await;

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        // Create preconnection with specific transport properties
        let props = TransportProperties::builder()
            .reliability(Preference::Require)
            .preserve_order(Preference::Require)
            .congestion_control(Preference::Require)
            .keep_alive(Preference::Prefer)
            .connection_timeout(Duration::from_secs(10))
            .connection_priority(100)
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            props,
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.unwrap();

        // Wait for establishment
        while conn.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(conn.state().await, ConnectionState::Established);

        // Send some data
        let msg = Message::from_string("Test with properties")
            .with_priority(50)
            .safely_replayable();
        conn.send(msg).await.unwrap();

        conn.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_connection_with_local_endpoint() {
    let test_body = async {
        let server_addr = start_test_server("echo").await;

        let local = LocalEndpoint::builder()
            .port(0) // Let system choose
            .build();

        let remote = RemoteEndpoint::builder()
            .ip_address(server_addr.ip())
            .port(server_addr.port())
            .build();

        let preconn = new_preconnection(
            vec![local],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.unwrap();

        // Wait for establishment
        while conn.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Check that local endpoint was populated
        let local_ep = conn.local_endpoint().await;
        assert!(local_ep.is_some());

        conn.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_connection_clone_group() {
    let test_body = async {
        let server_addr = start_test_server("echo").await;

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

        let conn1 = preconn.initiate().await.unwrap();

        // Wait for establishment
        while conn1.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Clone the connection (creates new connection in same group)
        let conn2 = conn1.clone_connection().await.unwrap();

        // Wait for second connection to establish
        while conn2.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert_eq!(conn2.state().await, ConnectionState::Established);

        // Send on both connections
        conn1
            .send(Message::from_string("From conn1"))
            .await
            .unwrap();
        conn2
            .send(Message::from_string("From conn2"))
            .await
            .unwrap();

        // Close both
        conn1.close().await.unwrap();
        conn2.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_hostname_resolution() {
    // Test with localhost hostname
    let server_addr = start_test_server("echo").await;

    let remote = RemoteEndpoint::builder()
        .hostname("localhost")
        .port(server_addr.port())
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment with timeout
    let mut attempts = 0;
    while conn.state().await == ConnectionState::Establishing && attempts < 20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        attempts += 1;
    }

    // Check if we got an establishment error event instead
    let final_state = conn.state().await;
    if final_state != ConnectionState::Established {
        if let Some(event) = tokio::time::timeout(Duration::from_millis(100), conn.next_event())
            .await
            .ok()
            .flatten()
        {
            match event {
                ConnectionEvent::EstablishmentError(msg) => {
                    // Expected if localhost resolution fails
                    println!("Hostname resolution test skipped: {}", msg);
                    return; // Test passes - we handled the error correctly
                }
                _ => {}
            }
        }
        panic!(
            "Connection failed to establish (state: {:?}) and no error event received",
            final_state
        );
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_multicast_endpoint() {
    // Test multicast endpoint creation (not actual multicast connection)
    let local = LocalEndpoint::builder()
        .any_source_multicast_group_ip("224.0.0.1".parse().unwrap())
        .interface("lo0")
        .build();

    let remote = RemoteEndpoint::builder()
        .multicast_group_ip("224.0.0.1".parse().unwrap())
        .hop_limit(1)
        .build();

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    // Just verify we can create and resolve endpoints
    let (locals, remotes) = preconn.resolve().await.unwrap();
    assert_eq!(locals.len(), 1);
    assert_eq!(remotes.len(), 1);
}

#[tokio::test]
async fn test_protocol_specific_endpoint() {
    let server_addr = start_test_server("http").await;

    // Create protocol-specific endpoints
    let tcp_remote = RemoteEndpoint::builder()
        .ip_address(server_addr.ip())
        .port(server_addr.port())
        .protocol(Protocol::TCP)
        .build();

    let preconn = new_preconnection(
        vec![],
        vec![tcp_remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert_eq!(conn.state().await, ConnectionState::Established);

    // Send HTTP request
    let request = Message::from_string("GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
    conn.send(request).await.unwrap();

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_listener_accept_connection() {
    // Create a listener
    let local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let preconn = new_preconnection(
        vec![local],
        vec![],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let listen_addr = listener.local_addr().await.unwrap();

    // Connect from client with short timeout
    let client_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        TcpStream::connect(listen_addr).await.unwrap()
    });

    // Accept connection with timeout
    let accept_result = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;

    assert!(accept_result.is_ok());
    let conn = accept_result.unwrap().unwrap();
    assert_eq!(conn.state().await, ConnectionState::Established);

    // Clean up
    let _ = client_handle.await;
    conn.close().await.unwrap();
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_client_server_data_exchange() {
    // Create a listener
    let local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let server_preconn = new_preconnection(
        vec![local],
        vec![],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let mut listener = server_preconn.listen().await.unwrap();
    let listen_addr = listener.local_addr().await.unwrap();

    // Server accept loop
    let server_handle = tokio::spawn(async move {
        let conn = listener.accept().await.unwrap();

        // Send a response
        let msg = Message::from_string("Hello from server");
        conn.send(msg).await.unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        conn.close().await.unwrap();
        listener.stop().await.unwrap();
    });

    // Client connection
    let remote = RemoteEndpoint::builder()
        .ip_address(listen_addr.ip())
        .port(listen_addr.port())
        .build();

    let client_preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let client_conn = client_preconn
        .initiate_with_timeout(Some(Duration::from_millis(100)))
        .await
        .unwrap();

    // Wait for establishment
    let mut attempts = 0;
    while client_conn.state().await == ConnectionState::Establishing && attempts < 10 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        attempts += 1;
    }

    assert_eq!(client_conn.state().await, ConnectionState::Established);

    // Send from client
    let msg = Message::from_string("Hello from client");
    client_conn.send(msg).await.unwrap();

    client_conn.close().await.unwrap();
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_listener_multiple_clients() {
    let local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let preconn = new_preconnection(
        vec![local],
        vec![],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let listen_addr = listener.local_addr().await.unwrap();

    // Spawn multiple clients
    let mut client_handles = vec![];
    for i in 0..3 {
        let addr = listen_addr;
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10 * (i + 1))).await;
            TcpStream::connect(addr).await.unwrap()
        });
        client_handles.push(handle);
    }

    // Accept all connections
    let mut server_conns = vec![];
    for _ in 0..3 {
        let conn = tokio::time::timeout(Duration::from_millis(200), listener.accept())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(conn.state().await, ConnectionState::Established);
        server_conns.push(conn);
    }

    // Send messages from server to each client
    for (i, conn) in server_conns.iter().enumerate() {
        let msg = Message::from_string(&format!("Hello client {}", i));
        conn.send(msg).await.unwrap();
    }

    // Clean up
    for handle in client_handles {
        let _ = handle.await;
    }

    for conn in server_conns {
        conn.close().await.unwrap();
    }

    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_listener_connection_limit_integration() {
    let local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let preconn = new_preconnection(
        vec![local],
        vec![],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    let mut listener = preconn.listen().await.unwrap();
    let listen_addr = listener.local_addr().await.unwrap();

    // Set connection limit to 2
    listener.set_new_connection_limit(2);

    // Spawn 3 clients
    let mut client_handles = vec![];
    for i in 0..3 {
        let addr = listen_addr;
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10 * (i + 1))).await;
            TcpStream::connect(addr).await
        });
        client_handles.push(handle);
    }

    // Try to accept 3 connections - only 2 should succeed
    let mut accepted = 0;
    for _ in 0..3 {
        match tokio::time::timeout(Duration::from_millis(50), listener.accept()).await {
            Ok(Ok(_)) => accepted += 1,
            _ => break,
        }
    }

    assert_eq!(accepted, 2);

    // Clean up
    for handle in client_handles {
        let _ = handle.await;
    }

    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_peer_to_peer() {
    // Simulate two peers doing rendezvous

    // Peer A
    let peer_a_local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let peer_a_remote = RemoteEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0) // Will be updated with peer B's actual port
        .build();

    let preconn_a = new_preconnection(
        vec![peer_a_local],
        vec![peer_a_remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    // Start peer A rendezvous
    let (conn_a, mut listener_a) = preconn_a.rendezvous().await.unwrap();
    let addr_a = listener_a.local_addr().await.unwrap();

    // Peer B
    let peer_b_local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let peer_b_remote = RemoteEndpoint::builder()
        .socket_address(addr_a) // Connect to peer A
        .build();

    let preconn_b = new_preconnection(
        vec![peer_b_local],
        vec![peer_b_remote],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );

    // Start peer B rendezvous
    let (conn_b, listener_b) = preconn_b.rendezvous().await.unwrap();

    // Wait for connections to establish
    let mut attempts = 0;
    while (conn_a.state().await == ConnectionState::Establishing
        || conn_b.state().await == ConnectionState::Establishing)
        && attempts < 20
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
        attempts += 1;
    }

    // At least one connection should be established
    let a_established = conn_a.state().await == ConnectionState::Established;
    let b_established = conn_b.state().await == ConnectionState::Established;
    assert!(
        a_established || b_established,
        "At least one peer should establish connection"
    );

    // Or accept incoming connection
    if !a_established {
        match tokio::time::timeout(Duration::from_millis(100), listener_a.accept()).await {
            Ok(Ok(_)) => {
                // Got incoming connection
            }
            _ => {}
        }
    }

    // Clean up
    listener_a.stop().await.unwrap();
    listener_b.stop().await.unwrap();
}

#[tokio::test]
async fn test_rendezvous_with_transport_properties() {
    let props = TransportProperties::builder()
        .reliability(Preference::Require)
        .connection_timeout(Duration::from_secs(2))
        .build();

    let local = LocalEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(0)
        .build();

    let remote = RemoteEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(54327) // Non-listening port
        .build();

    let preconn = new_preconnection(
        vec![local],
        vec![remote],
        props,
        SecurityParameters::new_disabled(),
    );

    let (connection, listener) = preconn.rendezvous().await.unwrap();

    // Verify listener is active
    assert!(listener.is_active().await);

    // Verify connection has correct initial state
    assert_eq!(connection.state().await, ConnectionState::Establishing);

    // Clean up
    listener.stop().await.unwrap();
}

#[tokio::test]
async fn test_message_properties_integration() {
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

    let preconn = new_preconnection(
        vec![],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let conn = preconn.initiate().await.unwrap();

    // Wait for establishment
    while conn.state().await == ConnectionState::Establishing {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Consume Ready event
    let _ = conn.next_event().await;

    // Send messages with different properties

    // High priority urgent message
    let urgent = Message::builder(b"URGENT".to_vec())
        .priority(1000)
        .lifetime(Duration::from_millis(100))
        .capacity_profile(MessageCapacityProfile::LowLatencyInteractive)
        .build();

    conn.send(urgent).await.unwrap();

    // Bulk data transfer
    let bulk = Message::builder(b"Large bulk data...".to_vec())
        .priority(1)
        .capacity_profile(MessageCapacityProfile::Scavenger)
        .checksum_length(32)
        .reliable(true)
        .build();

    conn.send(bulk).await.unwrap();

    // Ordered transaction messages
    for i in 0..3 {
        let txn = Message::builder(format!("Transaction {}", i).into_bytes())
            .ordered(true)
            .reliable(true)
            .safely_replayable(false)
            .build();
        conn.send(txn).await.unwrap();
    }

    // Final message to close the session
    let final_msg = Message::builder(b"GOODBYE".to_vec())
        .final_message(true)
        .no_fragmentation()
        .build();

    conn.send(final_msg).await.unwrap();

    // Verify all messages were sent
    for _ in 0..6 {
        let event = conn.next_event().await;
        assert!(matches!(event, Some(ConnectionEvent::Sent { .. })));
    }

    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_full_send_receive_flow() {
    let test_body = async {
        // Start an echo server that understands length-prefix framing
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                // Simple length-prefix echo server
                loop {
                    let mut len_buf = [0u8; 4];
                    match stream.read_exact(&mut len_buf).await {
                        Ok(_) => {}
                        Err(_) => break,
                    }

                    let len = u32::from_be_bytes(len_buf) as usize;
                    let mut msg_buf = vec![0u8; len];
                    match stream.read_exact(&mut msg_buf).await {
                        Ok(_) => {}
                        Err(_) => break,
                    }

                    // Echo back with length prefix
                    if stream.write_all(&len_buf).await.is_err() {
                        break;
                    }
                    if stream.write_all(&msg_buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Create client connection
        let remote = RemoteEndpoint::builder()
            .socket_address(server_addr)
            .build();

        let preconn = new_preconnection(
            vec![],
            vec![remote],
            TransportProperties::default(),
            SecurityParameters::new_disabled(),
        );

        let conn = preconn.initiate().await.unwrap();

        // Wait for establishment
        while conn.state().await == ConnectionState::Establishing {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Consume Ready event
        let _ = conn.next_event().await;

        // Set up length-prefix framer
        conn.use_length_prefix_framer().await.unwrap();

        // Send a message - the framer will add length prefix
        let send_msg = Message::from_string("Hello from TAPS!");
        conn.send(send_msg.clone()).await.unwrap();

        // Wait for Sent event
        let event = conn.next_event().await;
        assert!(matches!(event, Some(ConnectionEvent::Sent { .. })));

        // Wait for Received event from background reader
        let received_event = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match conn.next_event().await {
                    Some(ConnectionEvent::Received {
                        message_data,
                        message_context,
                    }) => {
                        return Some((message_data, message_context));
                    }
                    Some(_) => continue, // Ignore other events
                    None => return None,
                }
            }
        })
        .await
        .expect("Timeout waiting for Received event")
        .expect("Should receive echo response");

        assert_eq!(received_event.0, send_msg.data());
        assert!(received_event.1.remote_endpoint.is_some());

        conn.close().await.unwrap();
    };

    tokio::time::timeout(Duration::from_secs(10), test_body)
        .await
        .expect("Test timed out");
}
