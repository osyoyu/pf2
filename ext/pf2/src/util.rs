use core::mem::transmute;
use rb_sys::*;
use std::{ffi::c_void, u128};

// Convert str literal to C string literal
macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const std::ffi::c_char
    };
}
pub(crate) use cstr;

pub type RubyCFunc = unsafe extern "C" fn() -> VALUE;

// TODO: rewrite as macro
pub fn to_ruby_cfunc_with_no_args<T>(f: unsafe extern "C" fn(T) -> VALUE) -> RubyCFunc {
    unsafe { transmute::<unsafe extern "C" fn(T) -> VALUE, RubyCFunc>(f) }
}
pub fn to_ruby_cfunc_with_args<T, U, V>(f: unsafe extern "C" fn(T, U, V) -> VALUE) -> RubyCFunc {
    unsafe { transmute::<unsafe extern "C" fn(T, U, V) -> VALUE, RubyCFunc>(f) }
}

#[allow(non_snake_case)]
pub fn RTEST(v: VALUE) -> bool {
    v != Qfalse as VALUE && v != Qnil as VALUE
}

extern "C" {
    pub fn extract_si_value_sival_ptr(info: *mut libc::siginfo_t) -> *mut c_void;
    pub fn rb_ull2num(n: u128) -> VALUE;
}
