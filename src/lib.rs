use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use rustls_pki_types::CertificateDer;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

// Opaque structs for TAPS objects
pub struct Preconnection<'a> {
    local_endpoints: Vec<LocalEndpoint>,
    remote_endpoints: Vec<RemoteEndpoint>,
    transport_properties: TransportProperties,
    security_parameters: SecurityParameters<'a>,
}
pub struct Connection<'a> {
    tls_conn: tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
    _marker: std::marker::PhantomData<&'a ()>,
}
pub struct Listener;
pub struct MessageContext;

pub struct LocalEndpoint {
    interface: Option<String>,
    port: Option<u16>,
    ip: Option<IpAddr>,
}

pub struct RemoteEndpoint {
    host_name: Option<String>,
    port: Option<u16>,
    ip: Option<IpAddr>,
    service: Option<String>,
}

#[derive(Clone, Copy)]
pub enum Preference {
    Require,
    Prefer,
    NoPreference,
    Avoid,
    Prohibit,
}

pub struct TransportProperties {
    reliability: Preference,
    preserve_msg_boundaries: Preference,
    per_msg_reliability: Preference,
}

pub struct SecurityParameters<'a> {
    server_certificate: Option<CertificateDer<'a>>,
}

#[no_mangle]
pub extern "C" fn taps_preconnection_create<'a>() -> *mut Preconnection<'a> {
    let preconnection = Preconnection {
        local_endpoints: Vec::new(),
        remote_endpoints: Vec::new(),
        transport_properties: TransportProperties {
            reliability: Preference::Require,
            preserve_msg_boundaries: Preference::NoPreference,
            per_msg_reliability: Preference::NoPreference,
        },
        security_parameters: SecurityParameters {
            server_certificate: None,
        },
    };
    Box::into_raw(Box::new(preconnection))
}

#[no_mangle]
pub extern "C" fn taps_preconnection_free(preconnection: *mut Preconnection) {
    if !preconnection.is_null() {
        unsafe {
            drop(Box::from_raw(preconnection));
        }
    }
}

#[no_mangle]
pub extern "C" fn taps_remote_endpoint_create() -> *mut RemoteEndpoint {
    Box::into_raw(Box::new(RemoteEndpoint {
        host_name: None,
        port: None,
        ip: None,
        service: None,
    }))
}

#[no_mangle]
pub extern "C" fn taps_remote_endpoint_free(endpoint: *mut RemoteEndpoint) {
    if !endpoint.is_null() {
        unsafe {
            drop(Box::from_raw(endpoint));
        }
    }
}

#[no_mangle]
pub extern "C" fn taps_remote_endpoint_with_hostname(endpoint: *mut RemoteEndpoint, hostname: *const libc::c_char) {
    let endpoint = unsafe { &mut *endpoint };
    let hostname = unsafe { std::ffi::CStr::from_ptr(hostname).to_str().unwrap().to_string() };
    endpoint.host_name = Some(hostname);
}

#[no_mangle]
pub extern "C" fn taps_remote_endpoint_with_port(endpoint: *mut RemoteEndpoint, port: u16) {
    let endpoint = unsafe { &mut *endpoint };
    endpoint.port = Some(port);
}

#[no_mangle]
pub extern "C" fn taps_preconnection_add_remote_endpoint(preconnection: *mut Preconnection, endpoint: *mut RemoteEndpoint) {
    let preconnection = unsafe { &mut *preconnection };
    let endpoint = unsafe { Box::from_raw(endpoint) };
    preconnection.remote_endpoints.push(*endpoint);
}

#[no_mangle]
pub extern "C" fn taps_preconnection_require_reliability(preconnection: *mut Preconnection) {
    let preconnection = unsafe { &mut *preconnection };
    preconnection.transport_properties.reliability = Preference::Require;
}

#[no_mangle]
pub extern "C" fn taps_preconnection_prefer_reliability(preconnection: *mut Preconnection) {
    let preconnection = unsafe { &mut *preconnection };
    preconnection.transport_properties.reliability = Preference::Prefer;
}

#[no_mangle]
pub extern "C" fn taps_preconnection_avoid_reliability(preconnection: *mut Preconnection) {
    let preconnection = unsafe { &mut *preconnection };
    preconnection.transport_properties.reliability = Preference::Avoid;
}

#[no_mangle]
pub extern "C" fn taps_preconnection_prohibit_reliability(preconnection: *mut Preconnection) {
    let preconnection = unsafe { &mut *preconnection };
    preconnection.transport_properties.reliability = Preference::Prohibit;
}

#[no_mangle]
pub extern "C" fn taps_connection_initiate(preconnection: *mut Preconnection) -> *mut Connection {
    let preconnection = unsafe { Box::from_raw(preconnection) };
    RUNTIME.block_on(async {
        // For now, just connect to a hardcoded address
        let addr = "example.com:443";
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();

        let mut root_cert_store = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
            root_cert_store.add(cert).unwrap();
        }

        let config = tokio_rustls::TlsConnector::from(std::sync::Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth(),
        ));

        let domain = rustls_pki_types::ServerName::try_from("example.com").unwrap();
        let tls_conn = config.connect(domain.to_owned(), stream).await.unwrap();

        let conn_obj = Connection {
            tls_conn,
            _marker: std::marker::PhantomData,
        };

        Box::into_raw(Box::new(conn_obj))
    })
}

#[no_mangle]
pub extern "C" fn taps_connection_send(connection: *mut Connection, data: *const u8, len: libc::size_t) {
    let connection = unsafe { &mut *connection };
    let data = unsafe { std::slice::from_raw_parts(data, len as usize) };
    RUNTIME.block_on(async {
        connection.tls_conn.write_all(data).await.unwrap();
    });
}

#[no_mangle]
pub extern "C" fn taps_connection_receive(connection: *mut Connection, buffer: *mut u8, len: libc::size_t) -> libc::ssize_t {
    let connection = unsafe { &mut *connection };
    let mut buffer = unsafe { std::slice::from_raw_parts_mut(buffer, len as usize) };
    RUNTIME.block_on(async {
        match connection.tls_conn.read(&mut buffer).await {
            Ok(n) => n as libc::ssize_t,
            Err(_) => -1,
        }
    })
}

#[no_mangle]
pub extern "C" fn taps_connection_close(connection: *mut Connection) {
    if !connection.is_null() {
        let mut connection = unsafe { Box::from_raw(connection) };
        RUNTIME.block_on(async {
            connection.tls_conn.shutdown().await.unwrap();
        });
    }
}


#[no_mangle]
pub extern "C" fn taps_listener_listen(_preconnection: *mut Preconnection) -> *mut Listener {
    // Placeholder
    std::ptr::null_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let preconnection = taps_preconnection_create();
        assert!(!preconnection.is_null());
        taps_preconnection_free(preconnection);
    }
}
