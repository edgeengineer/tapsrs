//! Direct FFI bindings for Network.framework
//!
//! Since objc2 doesn't support Network.framework yet, we use direct C bindings.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use libc::{c_char, c_int, c_void};

// Opaque types
pub enum nw_path_monitor {}
pub type nw_path_monitor_t = *mut nw_path_monitor;

pub enum nw_path {}
pub type nw_path_t = *mut nw_path;

pub enum nw_interface {}
pub type nw_interface_t = *mut nw_interface;

pub enum nw_endpoint {}
pub type nw_endpoint_t = *mut nw_endpoint;

pub enum nw_protocol_options {}
pub type nw_protocol_options_t = *mut nw_protocol_options;

// Dispatch types
pub type dispatch_queue_t = *mut c_void;
pub type dispatch_block_t = *const c_void;

// Interface types
pub type nw_interface_type_t = c_int;
pub const NW_INTERFACE_TYPE_OTHER: nw_interface_type_t = 0;
pub const NW_INTERFACE_TYPE_WIFI: nw_interface_type_t = 1;
pub const NW_INTERFACE_TYPE_CELLULAR: nw_interface_type_t = 2;
pub const NW_INTERFACE_TYPE_WIRED: nw_interface_type_t = 3;
pub const NW_INTERFACE_TYPE_LOOPBACK: nw_interface_type_t = 4;

// Path status
pub type nw_path_status_t = c_int;
pub const NW_PATH_STATUS_INVALID: nw_path_status_t = 0;
pub const NW_PATH_STATUS_SATISFIED: nw_path_status_t = 1;
pub const NW_PATH_STATUS_UNSATISFIED: nw_path_status_t = 2;
pub const NW_PATH_STATUS_SATISFIABLE: nw_path_status_t = 3;

#[link(name = "Network", kind = "framework")]
extern "C" {
    // Path monitor functions
    pub fn nw_path_monitor_create() -> nw_path_monitor_t;
    pub fn nw_path_monitor_create_with_type(
        required_interface_type: nw_interface_type_t,
    ) -> nw_path_monitor_t;
    pub fn nw_path_monitor_set_queue(monitor: nw_path_monitor_t, queue: dispatch_queue_t);
    pub fn nw_path_monitor_start(monitor: nw_path_monitor_t);
    pub fn nw_path_monitor_cancel(monitor: nw_path_monitor_t);

    // Path functions
    pub fn nw_path_get_status(path: nw_path_t) -> nw_path_status_t;
    pub fn nw_path_is_expensive(path: nw_path_t) -> bool;
    pub fn nw_path_is_constrained(path: nw_path_t) -> bool;
    pub fn nw_path_uses_interface_type(
        path: nw_path_t,
        interface_type: nw_interface_type_t,
    ) -> bool;
    pub fn nw_path_enumerate_interfaces(path: nw_path_t, enumerate_block: dispatch_block_t)
        -> bool;

    // Interface functions
    pub fn nw_interface_get_type(interface: nw_interface_t) -> nw_interface_type_t;
    pub fn nw_interface_get_name(interface: nw_interface_t) -> *const c_char;
    pub fn nw_interface_get_index(interface: nw_interface_t) -> u32;

    // Object management
    pub fn nw_retain(obj: *mut c_void) -> *mut c_void;
    pub fn nw_release(obj: *mut c_void);
}

// Dispatch queue functions
#[link(name = "System", kind = "dylib")]
extern "C" {
    pub fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> dispatch_queue_t;
    pub fn dispatch_release(object: dispatch_queue_t);
}

// Block support for Objective-C blocks
use std::marker::PhantomData;
use std::mem;
use std::os::raw::c_ulong;

// Block structure for Objective-C blocks
#[repr(C)]
pub struct Block<F> {
    isa: *const c_void,
    flags: c_int,
    reserved: c_int,
    invoke: unsafe extern "C" fn(*mut Block<F>, nw_path_t),
    descriptor: *const BlockDescriptor<F>,
    closure: F,
}

#[repr(C)]
pub struct BlockDescriptor<F> {
    reserved: c_ulong,
    size: c_ulong,
    copy_helper: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    dispose_helper: Option<unsafe extern "C" fn(*mut c_void)>,
    signature: *const c_char,
    _phantom: PhantomData<F>,
}

impl<F> Block<F>
where
    F: FnMut(nw_path_t),
{
    pub fn new(closure: F) -> *mut Self {
        let descriptor = Box::new(BlockDescriptor::<F> {
            reserved: 0,
            size: mem::size_of::<Block<F>>() as c_ulong,
            copy_helper: Some(copy_helper::<F>),
            dispose_helper: Some(dispose_helper::<F>),
            signature: b"v@?@\0".as_ptr() as *const c_char, // void (^)(nw_path_t)
            _phantom: PhantomData,
        });

        let mut block = Box::new(Block {
            isa: unsafe { &_NSConcreteStackBlock as *const _ as *const c_void },
            flags: (1 << 25) | (1 << 24), // BLOCK_HAS_COPY_DISPOSE | BLOCK_HAS_SIGNATURE
            reserved: 0,
            invoke: invoke::<F>,
            descriptor: Box::into_raw(descriptor),
            closure,
        });

        // Copy to heap
        unsafe {
            let heap_block = _Block_copy(block.as_mut() as *mut _ as *const c_void);
            let _ = Box::into_raw(block); // Leak the stack block
            heap_block as *mut Self
        }
    }
}

unsafe extern "C" fn invoke<F>(block_ptr: *mut Block<F>, path: nw_path_t)
where
    F: FnMut(nw_path_t),
{
    let block = &mut *block_ptr;
    (block.closure)(path);
}

unsafe extern "C" fn copy_helper<F>(_dst: *mut c_void, _src: *const c_void) {
    // For our use case, we don't need to implement copy
}

unsafe extern "C" fn dispose_helper<F>(_block: *mut c_void) {
    // Cleanup will happen when block is released
}

extern "C" {
    static _NSConcreteStackBlock: c_void;
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    fn _Block_release(block: *const c_void);
}

// Wrapper for safe block handling
pub struct PathUpdateBlock {
    block: *mut c_void,
}

impl PathUpdateBlock {
    pub fn new<F>(closure: F) -> Self
    where
        F: FnMut(nw_path_t) + 'static,
    {
        let block = Block::new(closure);
        PathUpdateBlock {
            block: block as *mut c_void,
        }
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.block
    }
}

impl Drop for PathUpdateBlock {
    fn drop(&mut self) {
        unsafe {
            _Block_release(self.block);
        }
    }
}

unsafe impl Send for PathUpdateBlock {}

#[link(name = "Network", kind = "framework")]
extern "C" {
    pub fn nw_path_monitor_set_update_handler(monitor: nw_path_monitor_t, handler: *mut c_void);
}
