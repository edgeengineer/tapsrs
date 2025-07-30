//! Android platform implementation using JNI
//!
//! Uses ConnectivityManager for monitoring network changes.

use super::*;
use jni::{objects::{GlobalRef, JObject, JValue}, JNIEnv, JavaVM};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

static STATE: OnceLock<Arc<Mutex<State>>> = OnceLock::new();

struct State {
    watchers: HashMap<WatcherId, Box<dyn Fn(ChangeEvent) + Send + 'static>>,
    current_interfaces: Vec<Interface>,
    next_watcher_id: usize,
    java_support: Option<JavaSupport>,
}

struct JavaSupport {
    jvm: JavaVM,
    support_object: GlobalRef,
}

type WatcherId = usize;

pub struct AndroidMonitor {
    _phantom: std::marker::PhantomData<()>,
}

pub struct AndroidWatchHandle {
    id: WatcherId,
}

impl Drop for AndroidWatchHandle {
    fn drop(&mut self) {
        if let Some(state_ref) = STATE.get() {
            let mut state = state_ref.lock().unwrap();
            state.watchers.remove(&self.id);

            if state.watchers.is_empty() {
                if let Some(ref support) = state.java_support {
                    let _ = stop_java_watching(support);
                }
                state.java_support = None;
            }
        }
    }
}

impl PlatformMonitor for AndroidMonitor {
    fn list_interfaces(&self) -> Result<Vec<Interface>, Error> {
        // Get JVM and context from android_context
        let (vm_ptr, _context_ptr) = android_context()
            .ok_or_else(|| Error::PlatformError("Android context not set".into()))?;
        
        let jvm = unsafe { JavaVM::from_raw(vm_ptr as *mut jni::sys::JavaVM) }
            .map_err(|e| Error::PlatformError(format!("Failed to get JavaVM: {:?}", e)))?;
        
        let mut env = jvm.attach_current_thread()
            .map_err(|e| Error::PlatformError(format!("Failed to attach thread: {:?}", e)))?;
        
        // Call into Java to get network interfaces
        list_interfaces_jni(&mut env)
    }

    fn start_watching(
        &mut self,
        callback: Box<dyn Fn(ChangeEvent) + Send + 'static>,
    ) -> PlatformHandle {
        let state_ref = STATE.get_or_init(|| {
            Arc::new(Mutex::new(State {
                watchers: HashMap::new(),
                current_interfaces: Vec::new(),
                next_watcher_id: 1,
                java_support: None,
            }))
        });

        // Get current interfaces
        let current_list = match self.list_interfaces() {
            Ok(list) => list,
            Err(_) => Vec::new(),
        };

        // Send initial events for all current interfaces
        for interface in &current_list {
            callback(ChangeEvent::Added(interface.clone()));
        }

        let mut state = state_ref.lock().unwrap();
        let id = state.next_watcher_id;
        state.next_watcher_id += 1;
        state.current_interfaces = current_list;
        let is_first_watcher = state.watchers.is_empty();
        state.watchers.insert(id, callback);
        
        if is_first_watcher {
            let _ = start_java_watching(&mut state);
        }
        
        Box::new(AndroidWatchHandle { id })
    }
}

fn list_interfaces_jni(_env: &mut JNIEnv) -> Result<Vec<Interface>, Error> {
    // This would call into Java code to get the list of network interfaces
    // For now, return an empty list as this requires Java-side implementation
    Ok(Vec::new())
}

fn start_java_watching(state: &mut State) -> Result<(), Error> {
    let (vm_ptr, context_ptr) = android_context()
        .ok_or_else(|| Error::PlatformError("No Android context".into()))?;
    
    let jvm = unsafe { JavaVM::from_raw(vm_ptr as *mut jni::sys::JavaVM) }
        .map_err(|e| Error::PlatformError(format!("Failed to get JavaVM: {:?}", e)))?;
    
    let support_object = {
        let mut env = jvm.attach_current_thread()
            .map_err(|e| Error::PlatformError(format!("Failed to attach thread: {:?}", e)))?;
        
        // Create the Java support object
        let class_name = "com/transport_services/android/NetworkMonitorSupport";
        let support_class = env.find_class(class_name)
            .map_err(|e| Error::PlatformError(format!("Failed to find class: {:?}", e)))?;
        
        let constructor_sig = "(Landroid/content/Context;)V";
        let context_obj = unsafe { JObject::from_raw(context_ptr as jni::sys::jobject) };
        
        let support_object = env.new_object(&support_class, constructor_sig, &[(&context_obj).into()])
            .map_err(|e| Error::PlatformError(format!("Failed to create object: {:?}", e)))?;
        
        let global_ref = env.new_global_ref(support_object)
            .map_err(|e| Error::PlatformError(format!("Failed to create global ref: {:?}", e)))?;
        
        // Start watching with callback pointer
        let callback_ptr = transport_services_network_changed as *const () as jni::sys::jlong;
        env.call_method(
            &global_ref,
            "startNetworkWatch",
            "(J)V",
            &[JValue::Long(callback_ptr)],
        ).map_err(|e| Error::PlatformError(format!("Failed to start watch: {:?}", e)))?;
        
        global_ref
    };
    
    let java_support = JavaSupport {
        jvm,
        support_object,
    };
    state.java_support = Some(java_support);
    Ok(())
}

fn stop_java_watching(java_support: &JavaSupport) -> Result<(), Error> {
    let mut env = java_support.jvm.attach_current_thread()
        .map_err(|e| Error::PlatformError(format!("Failed to attach thread: {:?}", e)))?;
    
    env.call_method(
        &java_support.support_object,
        "stopNetworkWatch",
        "()V",
        &[],
    ).map_err(|e| Error::PlatformError(format!("Failed to stop watch: {:?}", e)))?;
    
    Ok(())
}

/// Called from Java when network interfaces change
#[no_mangle]
pub extern "C" fn transport_services_network_changed() {
    let Some(state_ref) = STATE.get() else {
        return;
    };
    
    // Get new interface list
    let new_list = match get_current_interfaces() {
        Ok(list) => list,
        Err(_) => return,
    };
    
    let mut state = state_ref.lock().unwrap();
    
    // Calculate diff
    let old_map: HashMap<String, &Interface> = state.current_interfaces
        .iter()
        .map(|i| (i.name.clone(), i))
        .collect();
    
    let new_map: HashMap<String, &Interface> = new_list
        .iter()
        .map(|i| (i.name.clone(), i))
        .collect();
    
    // Generate events
    let mut events = Vec::new();
    
    // Check for removed interfaces
    for (name, old_iface) in &old_map {
        if !new_map.contains_key(name) {
            events.push(ChangeEvent::Removed((*old_iface).clone()));
        }
    }
    
    // Check for added or modified interfaces
    for (name, new_iface) in &new_map {
        match old_map.get(name) {
            None => events.push(ChangeEvent::Added((*new_iface).clone())),
            Some(old_iface) => {
                if !interfaces_equal(old_iface, new_iface) {
                    events.push(ChangeEvent::Modified {
                        old: (*old_iface).clone(),
                        new: (*new_iface).clone(),
                    });
                }
            }
        }
    }
    
    // Update state and notify watchers
    state.current_interfaces = new_list;
    for event in events {
        for callback in state.watchers.values() {
            callback(event.clone());
        }
    }
}

fn get_current_interfaces() -> Result<Vec<Interface>, Error> {
    let (vm_ptr, _) = android_context()
        .ok_or_else(|| Error::PlatformError("No Android context".into()))?;
    
    let jvm = unsafe { JavaVM::from_raw(vm_ptr as *mut jni::sys::JavaVM) }
        .map_err(|e| Error::PlatformError(format!("Failed to get JavaVM: {:?}", e)))?;
    
    let mut env = jvm.attach_current_thread()
        .map_err(|e| Error::PlatformError(format!("Failed to attach thread: {:?}", e)))?;
    
    list_interfaces_jni(&mut env)
}

fn interfaces_equal(a: &Interface, b: &Interface) -> bool {
    a.name == b.name &&
    a.index == b.index &&
    a.ips == b.ips &&
    a.status == b.status &&
    a.interface_type == b.interface_type &&
    a.is_expensive == b.is_expensive
}

// Android context management
struct AndroidContext {
    vm: JavaVM,
    context: GlobalRef,
}

unsafe impl Send for AndroidContext {}
unsafe impl Sync for AndroidContext {}

static ANDROID_CONTEXT: OnceLock<Mutex<Option<AndroidContext>>> = OnceLock::new();

/// Sets the Android context for the transport services library.
///
/// # Safety
///
/// This function is unsafe because it accepts raw pointers from the JNI layer.
/// The caller must ensure that:
/// - `env` is a valid JNIEnv pointer from the current JNI call
/// - `context` is a valid jobject representing an Android Context
/// - The pointers remain valid for the duration of this function call
#[no_mangle]
pub unsafe extern "C" fn transport_services_set_android_context(
    env: *mut jni::sys::JNIEnv,
    context: jni::sys::jobject,
) -> i32 {
    match set_android_context_internal(env, context) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

unsafe fn set_android_context_internal(
    env: *mut jni::sys::JNIEnv,
    context: jni::sys::jobject,
) -> Result<(), Error> {
    let env = JNIEnv::from_raw(env)
        .map_err(|e| Error::PlatformError(format!("Invalid JNIEnv: {:?}", e)))?;
    let context_obj = JObject::from_raw(context);

    let jvm = env.get_java_vm()
        .map_err(|e| Error::PlatformError(format!("Failed to get JavaVM: {:?}", e)))?;
    let global_context = env.new_global_ref(context_obj)
        .map_err(|e| Error::PlatformError(format!("Failed to create global ref: {:?}", e)))?;

    let android_ctx = AndroidContext {
        vm: jvm,
        context: global_context,
    };

    let context_storage = ANDROID_CONTEXT.get_or_init(|| Mutex::new(None));
    *context_storage.lock().unwrap() = Some(android_ctx);

    Ok(())
}

fn android_context() -> Option<(*mut std::ffi::c_void, *mut std::ffi::c_void)> {
    if let Some(context_storage) = ANDROID_CONTEXT.get() {
        let ctx = context_storage.lock().unwrap();
        if let Some(ref android_ctx) = *ctx {
            let vm_ptr = android_ctx.vm.get_java_vm_pointer() as *mut std::ffi::c_void;
            let context_ptr = android_ctx.context.as_obj().as_raw() as *mut std::ffi::c_void;
            return Some((vm_ptr, context_ptr));
        }
    }
    None
}

pub fn create_platform_impl() -> Result<Box<dyn PlatformMonitor + Send + Sync>, Error> {
    Ok(Box::new(AndroidMonitor {
        _phantom: std::marker::PhantomData,
    }))
}