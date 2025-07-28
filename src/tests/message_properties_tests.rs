//! Unit tests for Message Properties functionality

use crate::{Message, MessageCapacityProfile};
use std::time::Duration;

#[test]
fn test_message_lifetime() {
    let msg = Message::from_string("Test").with_lifetime(Duration::from_secs(30));

    assert_eq!(msg.properties().lifetime, Some(Duration::from_secs(30)));
}

#[test]
fn test_message_priority() {
    let msg = Message::from_string("Test").with_priority(100);

    assert_eq!(msg.properties().priority, Some(100));

    // Test negative priority
    let low_priority = Message::from_string("Low priority").with_priority(-50);
    assert_eq!(low_priority.properties().priority, Some(-50));
}

#[test]
fn test_message_ordered() {
    let ordered_msg = Message::from_string("Ordered").with_ordered(true);
    assert_eq!(ordered_msg.properties().ordered, Some(true));

    let unordered_msg = Message::from_string("Unordered").with_ordered(false);
    assert_eq!(unordered_msg.properties().ordered, Some(false));

    // Default should be None
    let default_msg = Message::from_string("Default");
    assert_eq!(default_msg.properties().ordered, None);
}

#[test]
fn test_message_safely_replayable() {
    let replayable = Message::from_string("Replayable").safely_replayable();
    assert!(replayable.properties().safely_replayable);

    // Default should be false
    let default_msg = Message::from_string("Default");
    assert!(!default_msg.properties().safely_replayable);
}

#[test]
fn test_message_final() {
    let final_msg = Message::from_string("Final").final_message();
    assert!(final_msg.properties().final_message);

    // Default should be false
    let default_msg = Message::from_string("Default");
    assert!(!default_msg.properties().final_message);
}

#[test]
fn test_message_checksum_length() {
    let msg = Message::from_string("Checksummed").with_checksum_length(16); // 16 bytes = 128 bits
    assert_eq!(msg.properties().checksum_length, Some(16));

    // Default should be None
    let default_msg = Message::from_string("Default");
    assert_eq!(default_msg.properties().checksum_length, None);
}

#[test]
fn test_message_reliable() {
    let reliable = Message::from_string("Reliable").with_reliable(true);
    assert_eq!(reliable.properties().reliable, Some(true));

    let unreliable = Message::from_string("Unreliable").with_reliable(false);
    assert_eq!(unreliable.properties().reliable, Some(false));

    // Default should be None (inherit from connection)
    let default_msg = Message::from_string("Default");
    assert_eq!(default_msg.properties().reliable, None);
}

#[test]
fn test_message_capacity_profile() {
    let interactive = Message::from_string("Interactive")
        .with_capacity_profile(MessageCapacityProfile::LowLatencyInteractive);
    assert_eq!(
        interactive.properties().capacity_profile,
        Some(MessageCapacityProfile::LowLatencyInteractive)
    );

    let bulk =
        Message::from_string("Bulk").with_capacity_profile(MessageCapacityProfile::Scavenger);
    assert_eq!(
        bulk.properties().capacity_profile,
        Some(MessageCapacityProfile::Scavenger)
    );

    // Test all profiles
    let profiles = [
        MessageCapacityProfile::LowLatencyInteractive,
        MessageCapacityProfile::LowLatencyNonInteractive,
        MessageCapacityProfile::ConstantRate,
        MessageCapacityProfile::Scavenger,
    ];

    for profile in &profiles {
        let msg = Message::from_string("Test").with_capacity_profile(*profile);
        assert_eq!(msg.properties().capacity_profile, Some(*profile));
    }
}

#[test]
fn test_message_fragmentation_control() {
    let no_frag = Message::from_string("No fragmentation").no_fragmentation();
    assert!(no_frag.properties().no_fragmentation);

    let no_seg = Message::from_string("No segmentation").no_segmentation();
    assert!(no_seg.properties().no_segmentation);

    // Both can be set
    let both = Message::from_string("Both")
        .no_fragmentation()
        .no_segmentation();
    assert!(both.properties().no_fragmentation);
    assert!(both.properties().no_segmentation);

    // Default should be false for both
    let default_msg = Message::from_string("Default");
    assert!(!default_msg.properties().no_fragmentation);
    assert!(!default_msg.properties().no_segmentation);
}

#[test]
fn test_message_builder() {
    let msg = Message::builder(b"Test data".to_vec())
        .id(42)
        .lifetime(Duration::from_secs(60))
        .priority(10)
        .safely_replayable(true)
        .final_message(true)
        .ordered(true)
        .checksum_length(32)
        .reliable(true)
        .capacity_profile(MessageCapacityProfile::ConstantRate)
        .no_fragmentation()
        .no_segmentation()
        .end_of_message(true)
        .build();

    assert_eq!(msg.id(), Some(42));
    assert_eq!(msg.properties().lifetime, Some(Duration::from_secs(60)));
    assert_eq!(msg.properties().priority, Some(10));
    assert!(msg.properties().safely_replayable);
    assert!(msg.properties().final_message);
    assert_eq!(msg.properties().ordered, Some(true));
    assert_eq!(msg.properties().checksum_length, Some(32));
    assert_eq!(msg.properties().reliable, Some(true));
    assert_eq!(
        msg.properties().capacity_profile,
        Some(MessageCapacityProfile::ConstantRate)
    );
    assert!(msg.properties().no_fragmentation);
    assert!(msg.properties().no_segmentation);
    assert!(msg.is_end_of_message());
}

#[test]
fn test_message_builder_partial() {
    // Test builder with partial configuration
    let msg = Message::builder(b"Partial".to_vec())
        .priority(5)
        .ordered(false)
        .build();

    assert_eq!(msg.properties().priority, Some(5));
    assert_eq!(msg.properties().ordered, Some(false));

    // Other properties should have defaults
    assert_eq!(msg.properties().lifetime, None);
    assert!(!msg.properties().safely_replayable);
    assert!(!msg.properties().final_message);
    assert_eq!(msg.properties().checksum_length, None);
    assert_eq!(msg.properties().reliable, None);
    assert_eq!(msg.properties().capacity_profile, None);
    assert!(!msg.properties().no_fragmentation);
    assert!(!msg.properties().no_segmentation);
}

#[test]
fn test_combined_properties() {
    // Test combining multiple property setters
    let msg = Message::from_string("Complex message")
        .with_priority(100)
        .with_lifetime(Duration::from_millis(500))
        .safely_replayable()
        .with_ordered(true)
        .with_reliable(false)
        .with_checksum_length(8)
        .no_fragmentation();

    let props = msg.properties();
    assert_eq!(props.priority, Some(100));
    assert_eq!(props.lifetime, Some(Duration::from_millis(500)));
    assert!(props.safely_replayable);
    assert_eq!(props.ordered, Some(true));
    assert_eq!(props.reliable, Some(false));
    assert_eq!(props.checksum_length, Some(8));
    assert!(props.no_fragmentation);
    assert!(!props.no_segmentation);
}

#[test]
fn test_deprecated_idempotent() {
    // Test that deprecated idempotent method still works
    #[allow(deprecated)]
    let msg = Message::from_string("Test").idempotent();

    // Should set both the new and old fields
    assert!(msg.properties().safely_replayable);
    #[allow(deprecated)]
    {
        assert!(msg.properties().idempotent);
    }
}

#[test]
fn test_message_properties_clone() {
    let original = Message::builder(b"Original".to_vec())
        .priority(50)
        .lifetime(Duration::from_secs(30))
        .safely_replayable(true)
        .capacity_profile(MessageCapacityProfile::LowLatencyInteractive)
        .build();

    let cloned = original.clone();

    // Verify all properties are cloned
    assert_eq!(cloned.properties().priority, original.properties().priority);
    assert_eq!(cloned.properties().lifetime, original.properties().lifetime);
    assert_eq!(
        cloned.properties().safely_replayable,
        original.properties().safely_replayable
    );
    assert_eq!(
        cloned.properties().capacity_profile,
        original.properties().capacity_profile
    );
}
