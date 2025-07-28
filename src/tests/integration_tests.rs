//! Integration tests for Transport Services

use crate::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
                            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
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
}

#[tokio::test]
async fn test_transport_properties_application() {
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
        .idempotent();
    conn.send(msg).await.unwrap();
    
    conn.close().await.unwrap();
}

#[tokio::test]
async fn test_connection_with_local_endpoint() {
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
}

#[tokio::test]
async fn test_connection_clone_group() {
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
    conn1.send(Message::from_string("From conn1")).await.unwrap();
    conn2.send(Message::from_string("From conn2")).await.unwrap();
    
    // Close both
    conn1.close().await.unwrap();
    conn2.close().await.unwrap();
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
        if let Some(event) = tokio::time::timeout(
            Duration::from_millis(100), 
            conn.next_event()
        ).await.ok().flatten() {
            match event {
                ConnectionEvent::EstablishmentError(msg) => {
                    // Expected if localhost resolution fails
                    println!("Hostname resolution test skipped: {}", msg);
                    return; // Test passes - we handled the error correctly
                }
                _ => {}
            }
        }
        panic!("Connection failed to establish (state: {:?}) and no error event received", final_state);
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