//! FFI bindings for TransportProperties

use super::*;
use crate::{TransportProperties, TransportProperty, PropertyValue, Preference, MultipathConfig, CommunicationDirection};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::time::Duration;

/// Create a new TransportProperties object
#[no_mangle]
pub extern "C" fn transport_services_new_transport_properties() -> *mut TransportServicesHandle {
    let properties = Box::new(TransportProperties::new());
    to_handle(properties)
}

/// Free a TransportProperties object
#[no_mangle]
pub unsafe extern "C" fn transport_services_free_transport_properties(handle: *mut TransportServicesHandle) {
    if !handle.is_null() {
        let _ = from_handle::<TransportProperties>(handle);
    }
}

/// Set a preference property
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_preference(
    handle: *mut TransportServicesHandle,
    property: c_int,
    preference: types::TransportServicesPreference,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let pref: Preference = preference.into();
    
    let prop = match property {
        0 => TransportProperty::Reliability,
        1 => TransportProperty::PreserveMsgBoundaries,
        2 => TransportProperty::PerMsgReliability,
        3 => TransportProperty::PreserveOrder,
        4 => TransportProperty::ZeroRttMsg,
        5 => TransportProperty::Multistreaming,
        6 => TransportProperty::FullChecksumSend,
        7 => TransportProperty::FullChecksumRecv,
        8 => TransportProperty::CongestionControl,
        9 => TransportProperty::KeepAlive,
        10 => TransportProperty::UseTemporaryLocalAddress,
        11 => TransportProperty::SoftErrorNotify,
        12 => TransportProperty::ActiveReadBeforeSend,
        _ => return -1,
    };
    
    properties.set(prop, PropertyValue::Preference(pref));
    0
}

/// Set multipath configuration
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_multipath(
    handle: *mut TransportServicesHandle,
    config: types::TransportServicesMultipathConfig,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let mp_config: MultipathConfig = config.into();
    properties.set(TransportProperty::Multipath, PropertyValue::Multipath(mp_config));
    0
}

/// Set communication direction
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_direction(
    handle: *mut TransportServicesHandle,
    direction: types::TransportServicesCommunicationDirection,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let dir: CommunicationDirection = direction.into();
    properties.set(TransportProperty::Direction, PropertyValue::Direction(dir));
    0
}

/// Set advertises alternate address
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_advertises_altaddr(
    handle: *mut TransportServicesHandle,
    value: bool,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    properties.set(TransportProperty::AdvertisesAltaddr, PropertyValue::Bool(value));
    0
}

/// Set interface preference
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_interface(
    handle: *mut TransportServicesHandle,
    interface: *const c_char,
    preference: types::TransportServicesPreference,
) -> c_int {
    if handle.is_null() || interface.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let iface = match CStr::from_ptr(interface).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let pref: Preference = preference.into();
    
    properties.set(TransportProperty::Interface, PropertyValue::StringPreference(iface, pref));
    0
}

/// Set PVD preference
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_pvd(
    handle: *mut TransportServicesHandle,
    pvd: *const c_char,
    preference: types::TransportServicesPreference,
) -> c_int {
    if handle.is_null() || pvd.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let pvd_str = match CStr::from_ptr(pvd).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let pref: Preference = preference.into();
    
    properties.set(TransportProperty::Pvd, PropertyValue::StringPreference(pvd_str, pref));
    0
}

/// Set connection timeout in milliseconds
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_connection_timeout(
    handle: *mut TransportServicesHandle,
    timeout_ms: u64,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let duration = Duration::from_millis(timeout_ms);
    properties.set(TransportProperty::ConnectionTimeout, PropertyValue::Duration(duration));
    0
}

/// Set keep alive timeout in milliseconds
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_keep_alive_timeout(
    handle: *mut TransportServicesHandle,
    timeout_ms: u64,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    let duration = Duration::from_millis(timeout_ms);
    properties.set(TransportProperty::KeepAliveTimeout, PropertyValue::Duration(duration));
    0
}

/// Set connection priority
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_connection_priority(
    handle: *mut TransportServicesHandle,
    priority: i32,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let properties = handle_mut::<TransportProperties>(handle);
    properties.set(TransportProperty::ConnectionPriority, PropertyValue::Integer(priority));
    0
}

/// Property constants for FFI
pub mod property_constants {
    pub const TRANSPORT_SERVICES_PROPERTY_RELIABILITY: i32 = 0;
    pub const TRANSPORT_SERVICES_PROPERTY_PRESERVE_MSG_BOUNDARIES: i32 = 1;
    pub const TRANSPORT_SERVICES_PROPERTY_PER_MSG_RELIABILITY: i32 = 2;
    pub const TRANSPORT_SERVICES_PROPERTY_PRESERVE_ORDER: i32 = 3;
    pub const TRANSPORT_SERVICES_PROPERTY_ZERO_RTT_MSG: i32 = 4;
    pub const TRANSPORT_SERVICES_PROPERTY_MULTISTREAMING: i32 = 5;
    pub const TRANSPORT_SERVICES_PROPERTY_FULL_CHECKSUM_SEND: i32 = 6;
    pub const TRANSPORT_SERVICES_PROPERTY_FULL_CHECKSUM_RECV: i32 = 7;
    pub const TRANSPORT_SERVICES_PROPERTY_CONGESTION_CONTROL: i32 = 8;
    pub const TRANSPORT_SERVICES_PROPERTY_KEEP_ALIVE: i32 = 9;
    pub const TRANSPORT_SERVICES_PROPERTY_USE_TEMPORARY_LOCAL_ADDRESS: i32 = 10;
    pub const TRANSPORT_SERVICES_PROPERTY_SOFT_ERROR_NOTIFY: i32 = 11;
    pub const TRANSPORT_SERVICES_PROPERTY_ACTIVE_READ_BEFORE_SEND: i32 = 12;
}