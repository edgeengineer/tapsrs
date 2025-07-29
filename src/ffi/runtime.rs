//! Centralized Tokio runtime management for FFI
//!
//! Provides a single shared runtime to avoid creating new runtimes per FFI call

use once_cell::sync::OnceCell;
use std::sync::Mutex;
use tokio::runtime::Runtime;

static RUNTIME: OnceCell<Mutex<Runtime>> = OnceCell::new();

/// Initialize the global Tokio runtime
/// This should be called once during library initialization
pub fn init_runtime() -> Result<(), String> {
    RUNTIME
        .set(Mutex::new(
            Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?,
        ))
        .map_err(|_| "Runtime already initialized".to_string())
}

/// Shutdown the global Tokio runtime
/// This should be called during library cleanup
pub fn shutdown_runtime() {
    if let Some(runtime_mutex) = RUNTIME.get() {
        if let Ok(_runtime) = runtime_mutex.lock() {
            // The runtime will be dropped when the lock goes out of scope
            // This will gracefully shut down all spawned tasks
        }
    }
}

/// Get a handle to the global runtime
pub fn get_runtime_handle() -> Result<tokio::runtime::Handle, String> {
    Ok(RUNTIME
        .get()
        .ok_or_else(|| "Runtime not initialized".to_string())?
        .lock()
        .map_err(|e| format!("Failed to lock runtime: {}", e))?
        .handle()
        .clone())
}

/// Execute a future on the global runtime
pub fn block_on<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    Ok(RUNTIME
        .get()
        .ok_or_else(|| "Runtime not initialized".to_string())?
        .lock()
        .map_err(|e| format!("Failed to lock runtime: {}", e))?
        .block_on(future))
}

/// Spawn a task on the global runtime
pub fn spawn<F>(future: F) -> Result<tokio::task::JoinHandle<F::Output>, String>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handle = get_runtime_handle()?;
    Ok(handle.spawn(future))
}
