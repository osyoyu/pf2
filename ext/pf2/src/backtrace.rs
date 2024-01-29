use std::ffi::{c_char, c_int, CStr};

use libc::c_void;

#[derive(Debug)]
pub struct BacktraceState {
    ptr: *mut backtrace_sys2::backtrace_state,
}

unsafe impl Send for BacktraceState {}
unsafe impl Sync for BacktraceState {}

impl BacktraceState {
    pub unsafe fn new(ptr: *mut backtrace_sys2::backtrace_state) -> Self {
        Self { ptr }
    }

    pub fn as_mut_ptr(&self) -> *mut backtrace_sys2::backtrace_state {
        self.ptr
    }
}

pub struct Backtrace {}

impl Backtrace {
    pub fn backtrace_simple<F>(
        state: &BacktraceState,
        skip: i32,
        mut on_ok: F,
        on_error: backtrace_sys2::backtrace_error_callback,
    ) where
        F: FnMut(usize) -> c_int,
    {
        unsafe {
            backtrace_sys2::backtrace_simple(
                state.as_mut_ptr(),
                skip,
                Some(Self::backtrace_simple_trampoline::<F>),
                on_error,
                &mut on_ok as *mut _ as *mut c_void,
            );
        }
    }

    pub fn backtrace_pcinfo<F>(
        state: &BacktraceState,
        pc: usize,
        mut on_ok: F,
        on_error: backtrace_sys2::backtrace_error_callback,
    ) where
        F: FnMut(usize, *const c_char, c_int, *const c_char) -> c_int,
    {
        unsafe {
            backtrace_sys2::backtrace_pcinfo(
                state.as_mut_ptr(),
                pc,
                Some(Self::backtrace_full_trampoline::<F>),
                on_error,
                &mut on_ok as *mut _ as *mut c_void,
            );
        }
    }

    pub fn backtrace_syminfo<F>(
        state: &BacktraceState,
        pc: usize,
        mut on_ok: F,
        on_error: backtrace_sys2::backtrace_error_callback,
    ) where
        F: FnMut(usize, *const c_char, usize, usize),
    {
        unsafe {
            backtrace_sys2::backtrace_syminfo(
                state.as_mut_ptr(),
                pc,
                Some(Self::backtrace_syminfo_trampoline::<F>),
                on_error,
                &mut on_ok as *mut _ as *mut c_void,
            );
        }
    }

    unsafe extern "C" fn backtrace_full_trampoline<F>(
        user_data: *mut c_void,
        pc: usize,
        filename: *const c_char,
        lineno: c_int,
        function: *const c_char,
    ) -> c_int
    where
        F: FnMut(usize, *const c_char, c_int, *const c_char) -> c_int,
    {
        let user_data = &mut *(user_data as *mut F);
        user_data(pc, filename, lineno, function)
    }

    unsafe extern "C" fn backtrace_simple_trampoline<F>(user_data: *mut c_void, pc: usize) -> c_int
    where
        F: FnMut(usize) -> c_int,
    {
        let user_data = &mut *(user_data as *mut F);
        user_data(pc)
    }

    unsafe extern "C" fn backtrace_syminfo_trampoline<F>(
        user_data: *mut c_void,
        pc: usize,
        symname: *const c_char,
        symval: usize,
        symsize: usize,
    ) where
        F: FnMut(usize, *const c_char, usize, usize),
    {
        let user_data = &mut *(user_data as *mut F);
        user_data(pc, symname, symval, symsize);
    }

    pub unsafe extern "C" fn backtrace_error_callback(
        _data: *mut c_void,
        msg: *const c_char,
        errnum: c_int,
    ) {
        let msg = unsafe { CStr::from_ptr(msg) };
        log::debug!("backtrace error: {:?} ({})", msg, errnum);
    }
}
