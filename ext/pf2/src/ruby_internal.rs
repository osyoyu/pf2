#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use rb_sys::VALUE;

use std::ffi::{c_char, c_int, c_void};

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
