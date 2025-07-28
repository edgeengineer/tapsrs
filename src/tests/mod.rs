//! Tests for Transport Services implementation

#[cfg(test)]
mod connection_tests;

#[cfg(test)]
mod connection_group_tests;

#[cfg(test)]
mod preconnection_tests;

#[cfg(test)]
mod listener_tests;

#[cfg(test)]
mod rendezvous_tests;

#[cfg(test)]
mod message_sending_tests;

#[cfg(test)]
mod message_properties_tests;

#[cfg(test)]
mod connection_properties_tests;

#[cfg(test)]
mod endpoint_management_tests;

#[cfg(test)]
mod mtu_tests;

#[cfg(test)]
mod group_termination_tests;

#[cfg(test)]
mod property_updates_tests;

#[cfg(test)]
mod settable_properties_tests;

#[cfg(test)]
mod readonly_properties_tests;

#[cfg(test)]
mod tcp_properties_tests;

#[cfg(test)]
mod lifecycle_events_tests;

#[cfg(test)]
mod connection_termination_tests;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod background_reading_tests;
