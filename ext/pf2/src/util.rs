use core::mem::transmute;
use rb_sys::*;
use std::ffi::c_void;

// Convert str literal to C string literal
macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const std::ffi::c_char
    };
}
pub(crate) use cstr;

pub type RubyCFunc = unsafe extern "C" fn() -> VALUE;

// TODO: rewrite as macro
pub fn to_ruby_cfunc1<T>(f: unsafe extern "C" fn(T) -> VALUE) -> RubyCFunc {
    unsafe { transmute::<unsafe extern "C" fn(T) -> VALUE, RubyCFunc>(f) }
}
// TODO: rewrite as macro
pub fn to_ruby_cfunc2<T, U>(f: unsafe extern "C" fn(T, U) -> VALUE) -> RubyCFunc {
    unsafe { transmute::<unsafe extern "C" fn(T, U) -> VALUE, RubyCFunc>(f) }
}

extern "C" {
    pub fn extract_si_value_sival_ptr(info: *mut libc::siginfo_t) -> *mut c_void;
}
