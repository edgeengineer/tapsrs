//! Tests for all settable generic connection properties (RFC 8.1)

use crate::*;
use tokio::net::TcpListener;
use std::time::Duration;

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
        vec![RemoteEndpoint::builder()
            .socket_address(addr)
            .build()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    
    let conn = preconn.initiate().await.expect("Should connect");
    
    // Wait for ready
    match conn.next_event().await {
        Some(ConnectionEvent::Ready) => {},
        other => panic!("Expected Ready event, got {:?}", other),
    }
    
    conn
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recv_checksum_len_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test setting full coverage
        conn.set_property("recvChecksumLen", 
            ConnectionProperty::RecvChecksumLen(ChecksumCoverage::FullCoverage))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::RecvChecksumLen(coverage)) = conn.get_property("recvChecksumLen").await {
            assert_eq!(coverage, ChecksumCoverage::FullCoverage);
        } else {
            panic!("Property not found");
        }
        
        // Test setting minimum bytes
        conn.set_property("recvChecksumLen", 
            ConnectionProperty::RecvChecksumLen(ChecksumCoverage::MinBytes(1024)))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::RecvChecksumLen(coverage)) = conn.get_property("recvChecksumLen").await {
            assert_eq!(coverage, ChecksumCoverage::MinBytes(1024));
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conn_priority_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test setting different priorities
        let priorities = vec![0, 1, 50, 100, 255, 1000];
        
        for priority in priorities {
            conn.set_property("connPriority", ConnectionProperty::ConnPriority(priority))
                .await
                .expect("Should set property");
            
            if let Some(ConnectionProperty::ConnPriority(p)) = conn.get_property("connPriority").await {
                assert_eq!(p, priority);
            } else {
                panic!("Property not found");
            }
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conn_timeout_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test setting duration timeout
        let timeout_duration = Duration::from_secs(30);
        conn.set_property("connTimeout", 
            ConnectionProperty::ConnTimeout(TimeoutValue::Duration(timeout_duration)))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::ConnTimeout(timeout)) = conn.get_property("connTimeout").await {
            match timeout {
                TimeoutValue::Duration(d) => assert_eq!(d, timeout_duration),
                _ => panic!("Expected Duration timeout"),
            }
        } else {
            panic!("Property not found");
        }
        
        // Test disabling timeout
        conn.set_property("connTimeout", 
            ConnectionProperty::ConnTimeout(TimeoutValue::Disabled))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::ConnTimeout(timeout)) = conn.get_property("connTimeout").await {
            assert!(matches!(timeout, TimeoutValue::Disabled));
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_keepalive_timeout_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test setting keep-alive timeout
        let keepalive_duration = Duration::from_secs(60);
        conn.set_property("keepAliveTimeout", 
            ConnectionProperty::KeepAliveTimeout(TimeoutValue::Duration(keepalive_duration)))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::KeepAliveTimeout(timeout)) = conn.get_property("keepAliveTimeout").await {
            match timeout {
                TimeoutValue::Duration(d) => assert_eq!(d, keepalive_duration),
                _ => panic!("Expected Duration timeout"),
            }
        } else {
            panic!("Property not found");
        }
        
        // Test disabling keep-alive
        conn.set_property("keepAliveTimeout", 
            ConnectionProperty::KeepAliveTimeout(TimeoutValue::Disabled))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::KeepAliveTimeout(timeout)) = conn.get_property("keepAliveTimeout").await {
            assert!(matches!(timeout, TimeoutValue::Disabled));
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conn_scheduler_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test all scheduler types
        let schedulers = vec![
            SchedulerType::WeightedFairQueueing,
            SchedulerType::Fifo,
            SchedulerType::RoundRobin,
            SchedulerType::ProportionalRate,
        ];
        
        for scheduler in schedulers {
            conn.set_property("connScheduler", ConnectionProperty::ConnScheduler(scheduler))
                .await
                .expect("Should set property");
            
            if let Some(ConnectionProperty::ConnScheduler(s)) = conn.get_property("connScheduler").await {
                assert_eq!(s, scheduler);
            } else {
                panic!("Property not found");
            }
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conn_capacity_profile_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test all capacity profiles
        let profiles = vec![
            CapacityProfile::Default,
            CapacityProfile::LowLatencyInteractive,
            CapacityProfile::LowLatencyNonInteractive,
            CapacityProfile::ConstantRateStreaming,
            CapacityProfile::CapacitySeeking,
        ];
        
        for profile in profiles {
            conn.set_property("connCapacityProfile", ConnectionProperty::ConnCapacityProfile(profile))
                .await
                .expect("Should set property");
            
            if let Some(ConnectionProperty::ConnCapacityProfile(p)) = conn.get_property("connCapacityProfile").await {
                assert_eq!(p, profile);
            } else {
                panic!("Property not found");
            }
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_multipath_policy_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test all multipath policies
        let policies = vec![
            MultipathPolicy::Handover,
            MultipathPolicy::Active,
            MultipathPolicy::Redundant,
        ];
        
        for policy in policies {
            conn.set_property("multipathPolicy", ConnectionProperty::MultipathPolicy(policy))
                .await
                .expect("Should set property");
            
            if let Some(ConnectionProperty::MultipathPolicy(p)) = conn.get_property("multipathPolicy").await {
                assert_eq!(p, policy);
            } else {
                panic!("Property not found");
            }
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_send_rate_bounds_properties() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test minimum send rate
        conn.set_property("minSendRate", ConnectionProperty::MinSendRate(Some(1_000_000))) // 1 Mbps
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MinSendRate(rate)) = conn.get_property("minSendRate").await {
            assert_eq!(rate, Some(1_000_000));
        } else {
            panic!("Property not found");
        }
        
        // Test maximum send rate
        conn.set_property("maxSendRate", ConnectionProperty::MaxSendRate(Some(10_000_000))) // 10 Mbps
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MaxSendRate(rate)) = conn.get_property("maxSendRate").await {
            assert_eq!(rate, Some(10_000_000));
        } else {
            panic!("Property not found");
        }
        
        // Test unlimited rates
        conn.set_property("minSendRate", ConnectionProperty::MinSendRate(None))
            .await
            .expect("Should set property");
        
        conn.set_property("maxSendRate", ConnectionProperty::MaxSendRate(None))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MinSendRate(rate)) = conn.get_property("minSendRate").await {
            assert_eq!(rate, None);
        } else {
            panic!("Property not found");
        }
        
        if let Some(ConnectionProperty::MaxSendRate(rate)) = conn.get_property("maxSendRate").await {
            assert_eq!(rate, None);
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recv_rate_bounds_properties() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test minimum receive rate
        conn.set_property("minRecvRate", ConnectionProperty::MinRecvRate(Some(2_000_000))) // 2 Mbps
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MinRecvRate(rate)) = conn.get_property("minRecvRate").await {
            assert_eq!(rate, Some(2_000_000));
        } else {
            panic!("Property not found");
        }
        
        // Test maximum receive rate
        conn.set_property("maxRecvRate", ConnectionProperty::MaxRecvRate(Some(20_000_000))) // 20 Mbps
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MaxRecvRate(rate)) = conn.get_property("maxRecvRate").await {
            assert_eq!(rate, Some(20_000_000));
        } else {
            panic!("Property not found");
        }
        
        // Test unlimited rates
        conn.set_property("minRecvRate", ConnectionProperty::MinRecvRate(None))
            .await
            .expect("Should set property");
        
        conn.set_property("maxRecvRate", ConnectionProperty::MaxRecvRate(None))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::MinRecvRate(rate)) = conn.get_property("minRecvRate").await {
            assert_eq!(rate, None);
        } else {
            panic!("Property not found");
        }
        
        if let Some(ConnectionProperty::MaxRecvRate(rate)) = conn.get_property("maxRecvRate").await {
            assert_eq!(rate, None);
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_group_conn_limit_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test setting specific limit
        conn.set_property("groupConnLimit", ConnectionProperty::GroupConnLimit(Some(10)))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::GroupConnLimit(limit)) = conn.get_property("groupConnLimit").await {
            assert_eq!(limit, Some(10));
        } else {
            panic!("Property not found");
        }
        
        // Test unlimited
        conn.set_property("groupConnLimit", ConnectionProperty::GroupConnLimit(None))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::GroupConnLimit(limit)) = conn.get_property("groupConnLimit").await {
            assert_eq!(limit, None);
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_isolate_session_property() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Test enabling session isolation
        conn.set_property("isolateSession", ConnectionProperty::IsolateSession(true))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::IsolateSession(isolated)) = conn.get_property("isolateSession").await {
            assert_eq!(isolated, true);
        } else {
            panic!("Property not found");
        }
        
        // Test disabling session isolation
        conn.set_property("isolateSession", ConnectionProperty::IsolateSession(false))
            .await
            .expect("Should set property");
        
        if let Some(ConnectionProperty::IsolateSession(isolated)) = conn.get_property("isolateSession").await {
            assert_eq!(isolated, false);
        } else {
            panic!("Property not found");
        }
    }).await.expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_all_properties_defaults() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let conn = create_test_connection().await;
        
        // Get all properties and verify defaults
        let props = conn.get_properties().await;
        
        // Check all defaults as defined in ConnectionProperties::new()
        if let Some(ConnectionProperty::RecvChecksumLen(coverage)) = props.get("recvChecksumLen") {
            assert_eq!(*coverage, ChecksumCoverage::FullCoverage);
        } else {
            panic!("recvChecksumLen not found");
        }
        
        if let Some(ConnectionProperty::ConnPriority(priority)) = props.get("connPriority") {
            assert_eq!(*priority, 100); // Default priority
        } else {
            panic!("connPriority not found");
        }
        
        if let Some(ConnectionProperty::ConnTimeout(timeout)) = props.get("connTimeout") {
            assert!(matches!(timeout, TimeoutValue::Disabled));
        } else {
            panic!("connTimeout not found");
        }
        
        if let Some(ConnectionProperty::KeepAliveTimeout(timeout)) = props.get("keepAliveTimeout") {
            assert!(matches!(timeout, TimeoutValue::Disabled));
        } else {
            panic!("keepAliveTimeout not found");
        }
        
        if let Some(ConnectionProperty::ConnScheduler(scheduler)) = props.get("connScheduler") {
            assert_eq!(*scheduler, SchedulerType::WeightedFairQueueing);
        } else {
            panic!("connScheduler not found");
        }
        
        if let Some(ConnectionProperty::ConnCapacityProfile(profile)) = props.get("connCapacityProfile") {
            assert_eq!(*profile, CapacityProfile::Default);
        } else {
            panic!("connCapacityProfile not found");
        }
        
        if let Some(ConnectionProperty::MultipathPolicy(policy)) = props.get("multipathPolicy") {
            assert_eq!(*policy, MultipathPolicy::Handover);
        } else {
            panic!("multipathPolicy not found");
        }
        
        if let Some(ConnectionProperty::MinSendRate(rate)) = props.get("minSendRate") {
            assert_eq!(*rate, None); // Unlimited
        } else {
            panic!("minSendRate not found");
        }
        
        if let Some(ConnectionProperty::MaxSendRate(rate)) = props.get("maxSendRate") {
            assert_eq!(*rate, None); // Unlimited
        } else {
            panic!("maxSendRate not found");
        }
        
        if let Some(ConnectionProperty::MinRecvRate(rate)) = props.get("minRecvRate") {
            assert_eq!(*rate, None); // Unlimited
        } else {
            panic!("minRecvRate not found");
        }
        
        if let Some(ConnectionProperty::MaxRecvRate(rate)) = props.get("maxRecvRate") {
            assert_eq!(*rate, None); // Unlimited
        } else {
            panic!("maxRecvRate not found");
        }
        
        if let Some(ConnectionProperty::GroupConnLimit(limit)) = props.get("groupConnLimit") {
            assert_eq!(*limit, None); // Unlimited
        } else {
            panic!("groupConnLimit not found");
        }
        
        if let Some(ConnectionProperty::IsolateSession(isolated)) = props.get("isolateSession") {
            assert_eq!(*isolated, false); // Default is false
        } else {
            panic!("isolateSession not found");
        }
    }).await.expect("Test should complete within timeout");
}