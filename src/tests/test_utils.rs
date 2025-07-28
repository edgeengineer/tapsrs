//! Test utilities

/// Macro to add a timeout to async tests
#[macro_export]
macro_rules! test_timeout {
    ($duration_secs:expr, $body:expr) => {
        match tokio::time::timeout(std::time::Duration::from_secs($duration_secs), $body).await {
            Ok(result) => result,
            Err(_) => panic!("Test timed out after {} seconds", $duration_secs),
        }
    };
}

pub use test_timeout;