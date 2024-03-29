#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use libc::{clockid_t, pthread_t};
use rb_sys::{rb_check_typeddata, rb_data_type_struct, RTypedData, VALUE};
use std::ffi::{c_char, c_int, c_void};
use std::mem::MaybeUninit;

#[cfg(target_os = "linux")]
use libc::pthread_getcpuclockid;

#[cfg(not(target_os = "linux"))]
pub unsafe fn pthread_getcpuclockid(thread: pthread_t, clk_id: *mut clockid_t) -> c_int {
    unimplemented!()
}

// Types and structs from Ruby 3.4.0.
#[repr(C)]
pub struct rb_callable_method_entry_struct {
    /* same fields with rb_method_entry_t */
    pub flags: VALUE,
    _padding_defined_class: VALUE,
    pub def: *mut rb_method_definition_struct,
    // ...
}

#[repr(C)]
pub struct rb_method_definition_struct {
    pub type_: c_int,
    _padding: [c_char; 4],
    pub cfunc: rb_method_cfunc_struct,
    // ...
}

#[repr(C)]
pub struct rb_method_cfunc_struct {
    pub func: *mut c_void,
    // ...
}

type rb_nativethread_id_t = libc::pthread_t;

#[repr(C)]
struct rb_native_thread {
    _padding_serial: [c_char; 4], // rb_atomic_t
    _padding_vm: *mut c_int,      // struct rb_vm_struct
    thread_id: rb_nativethread_id_t,
    // ...
}

#[repr(C)]
struct rb_thread_struct {
    _padding_lt_node: [c_char; 16], // struct ccan_list_node
    _padding_self: VALUE,
    _padding_ractor: *mut c_int, // rb_ractor_t
    _padding_vm: *mut c_int,     // rb_vm_t
    nt: *mut rb_native_thread,
    // ...
}
type rb_thread_t = rb_thread_struct;

/// Reimplementation of the internal RTYPEDDATA_TYPE macro.
unsafe fn RTYPEDDATA_TYPE(obj: VALUE) -> *const rb_data_type_struct {
    let typed: *mut RTypedData = obj as *mut RTypedData;
    (*typed).type_
}

unsafe fn rb_thread_ptr(thread: VALUE) -> *mut rb_thread_t {
    unsafe { rb_check_typeddata(thread, RTYPEDDATA_TYPE(thread)) as *mut rb_thread_t }
}

pub unsafe fn rb_thread_getcpuclockid(thread: VALUE) -> clockid_t {
    let mut cid: clockid_t = MaybeUninit::zeroed().assume_init();
    let pthread_id: pthread_t = (*(*rb_thread_ptr(thread)).nt).thread_id;
    pthread_getcpuclockid(pthread_id, &mut cid as *mut clockid_t);
    cid
}
