//! Tests for connection properties functionality

use crate::*;
use std::time::Duration;

#[tokio::test]
async fn test_set_and_get_properties() {
    // Create a connection for testing
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    // Test setting connection priority
    conn.set_property("connPriority", ConnectionProperty::ConnPriority(50))
        .await
        .expect("Should set property");

    // Get the property back
    if let Some(prop) = conn.get_property("connPriority").await {
        match prop {
            ConnectionProperty::ConnPriority(val) => assert_eq!(val, 50),
            _ => panic!("Wrong property type returned"),
        }
    } else {
        panic!("Property not found");
    }
}

#[tokio::test]
async fn test_connection_properties_defaults() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    let props = conn.get_properties().await;

    // Check default values
    if let Some(ConnectionProperty::ConnPriority(val)) = props.get("connPriority") {
        assert_eq!(*val, 100); // Default priority
    } else {
        panic!("Default connPriority not set");
    }

    if let Some(ConnectionProperty::IsolateSession(val)) = props.get("isolateSession") {
        assert_eq!(*val, false); // Default
    } else {
        panic!("Default isolateSession not set");
    }

    if let Some(ConnectionProperty::ConnScheduler(val)) = props.get("connScheduler") {
        assert_eq!(*val, SchedulerType::WeightedFairQueueing); // Default
    } else {
        panic!("Default connScheduler not set");
    }
}

#[tokio::test]
async fn test_readonly_properties() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    let props = conn.get_properties().await;

    // Check read-only properties
    if let Some(ConnectionProperty::ConnState(state)) = props.get("connState") {
        assert_eq!(*state, ConnectionState::Established);
    } else {
        panic!("connState not found");
    }

    if let Some(ConnectionProperty::CanSend(val)) = props.get("canSend") {
        assert_eq!(*val, true); // Can send when established
    } else {
        panic!("canSend not found");
    }

    if let Some(ConnectionProperty::CanReceive(val)) = props.get("canReceive") {
        assert_eq!(*val, true); // Can receive when established
    } else {
        panic!("canReceive not found");
    }
}

#[tokio::test]
async fn test_readonly_property_rejection() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    // Try to set a read-only property
    let result = conn
        .set_property(
            "connState",
            ConnectionProperty::ConnState(ConnectionState::Closed),
        )
        .await;
    assert!(result.is_err());

    if let Err(e) = result {
        match e {
            TransportServicesError::InvalidParameters(msg) => {
                assert!(msg.contains("read-only"));
            }
            _ => panic!("Wrong error type"),
        }
    }
}

#[tokio::test]
async fn test_timeout_properties() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    // Set connection timeout
    let timeout = Duration::from_secs(30);
    conn.set_property(
        "connTimeout",
        ConnectionProperty::ConnTimeout(TimeoutValue::Duration(timeout)),
    )
    .await
    .expect("Should set timeout");

    // Get it back
    if let Some(ConnectionProperty::ConnTimeout(val)) = conn.get_property("connTimeout").await {
        match val {
            TimeoutValue::Duration(d) => assert_eq!(d, timeout),
            _ => panic!("Wrong timeout value"),
        }
    } else {
        panic!("Timeout property not found");
    }
}

#[tokio::test]
async fn test_capacity_profile() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    // Set capacity profile
    conn.set_property(
        "connCapacityProfile",
        ConnectionProperty::ConnCapacityProfile(CapacityProfile::LowLatencyInteractive),
    )
    .await
    .expect("Should set profile");

    // Check it was set
    if let Some(ConnectionProperty::ConnCapacityProfile(val)) =
        conn.get_property("connCapacityProfile").await
    {
        assert_eq!(val, CapacityProfile::LowLatencyInteractive);
    } else {
        panic!("Capacity profile not found");
    }
}

#[tokio::test]
async fn test_rate_limits() {
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
        ConnectionState::Established,
        None,
        None,
        TransportProperties::default(),
    );

    // Set max send rate to 1 Mbps
    conn.set_property(
        "maxSendRate",
        ConnectionProperty::MaxSendRate(Some(1_000_000)),
    )
    .await
    .expect("Should set rate");

    // Check it
    if let Some(ConnectionProperty::MaxSendRate(val)) = conn.get_property("maxSendRate").await {
        assert_eq!(val, Some(1_000_000));
    } else {
        panic!("Max send rate not found");
    }

    // Check default (unlimited)
    if let Some(ConnectionProperty::MinSendRate(val)) = conn.get_property("minSendRate").await {
        assert_eq!(val, None); // None means unlimited
    } else {
        panic!("Min send rate not found");
    }
}
