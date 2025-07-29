use crate::*;

#[tokio::test]
async fn test_new_preconnection() {
    let preconn = Preconnection::new(
        vec![],
        vec![],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    assert!(preconn.resolve().await.is_ok());
}

#[tokio::test]
async fn test_preconnection_with_endpoints() {
    let local = LocalEndpoint::builder()
        .interface("lo0")
        .port(0) // Let system choose port
        .build();

    let remote = RemoteEndpoint::builder()
        .ip_address("127.0.0.1".parse().unwrap())
        .port(8080)
        .build();

    let preconn = Preconnection::new(
        vec![local],
        vec![remote],
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let (locals, remotes) = preconn.resolve().await.unwrap();
    assert_eq!(locals.len(), 1);
    assert_eq!(remotes.len(), 1);
}

#[tokio::test]
async fn test_initiate_without_remote_endpoint_fails() {
    let preconn = Preconnection::new(
        vec![],
        vec![], // No remote endpoints
        TransportProperties::default(),
        SecurityParameters::default(),
    );

    let result = preconn.initiate().await;
    assert!(result.is_err());

    if let Err(e) = result {
        match e {
            TransportServicesError::InvalidParameters(msg) => {
                assert!(msg.contains("No remote endpoints"));
            }
            _ => panic!("Expected InvalidParameters error"),
        }
    }
}

#[tokio::test]
async fn test_preconnection_builder_pattern() {
    let preconn = Preconnection::with_remote_endpoint(
        RemoteEndpoint::builder()
            .hostname("example.com")
            .port(443)
            .service("https")
            .build(),
    );

    // Add local endpoint
    preconn
        .add_local(LocalEndpoint::builder().interface("en0").build())
        .await;

    // Set transport properties
    let props = TransportProperties::builder()
        .reliability(Preference::Require)
        .preserve_order(Preference::Require)
        .congestion_control(Preference::Require)
        .build();

    preconn.set_transport_properties(props).await;

    // Verify we can resolve endpoints
    let (locals, remotes) = preconn.resolve().await.unwrap();
    assert_eq!(locals.len(), 1);
    // Remote endpoint with hostname may resolve to multiple IPs
    assert!(!remotes.is_empty());
}

#[tokio::test]
async fn test_security_parameters() {
    let remote = RemoteEndpoint::builder()
        .hostname("example.com")
        .port(443)
        .build();

    // Test with disabled security
    let preconn = Preconnection::new(
        vec![],
        vec![remote.clone()],
        TransportProperties::default(),
        SecurityParameters::new_disabled(),
    );
    assert!(preconn.resolve().await.is_ok());

    // Test with opportunistic security
    let preconn = Preconnection::new(
        vec![],
        vec![remote.clone()],
        TransportProperties::default(),
        SecurityParameters::new_opportunistic(),
    );
    assert!(preconn.resolve().await.is_ok());

    // Test with custom security parameters
    let mut sec_params = SecurityParameters::new();
    sec_params.set(
        SecurityParameter::AllowedProtocols,
        SecurityParameterValue::Protocols(vec![SecurityProtocol::TLS13]),
    );
    sec_params.set(
        SecurityParameter::Alpn,
        SecurityParameterValue::Strings(vec!["h2".to_string(), "http/1.1".to_string()]),
    );

    let preconn = Preconnection::new(
        vec![],
        vec![remote],
        TransportProperties::default(),
        sec_params,
    );
    assert!(preconn.resolve().await.is_ok());
}
