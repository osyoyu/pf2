use std::ffi::{c_int, c_void, CStr};
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr::{addr_of, null_mut};

use rb_sys::*;

use crate::util::cstr;

use super::Session;

pub struct SessionRubyObject {
    session: Option<Session>,
}

impl SessionRubyObject {
    pub unsafe extern "C" fn rb_initialize(
        argc: c_int,
        argv: *const VALUE,
        rbself: VALUE,
    ) -> VALUE {
        let mut obj = unsafe { Self::get_struct_from(rbself) };
        obj.session = Some(Session::new_from_rb_initialize(argc, argv, rbself));
        Qnil.into()
    }

    pub unsafe extern "C" fn rb_start(rbself: VALUE) -> VALUE {
        let mut obj = Self::get_struct_from(rbself);
        match &mut obj.session {
            Some(session) => session.start(),
            None => panic!("Session is not initialized"),
        }
    }

    pub unsafe extern "C" fn rb_stop(rbself: VALUE) -> VALUE {
        let mut obj = Self::get_struct_from(rbself);
        match &mut obj.session {
            Some(session) => session.stop(),
            None => panic!("Session is not initialized"),
        }
    }

    pub unsafe extern "C" fn rb_mark(argc: c_int, argv: *mut VALUE, rbself: VALUE) -> VALUE {
        if argc != 1 {
            rb_raise(rb_eArgError, cstr!("number of arguments do not match"));
        }
        let args = unsafe { std::slice::from_raw_parts_mut(argv, argc.try_into().unwrap()) };
        let tag = CStr::from_ptr(rb_string_value_cstr(&mut args[0])).to_str().unwrap().to_owned();
        let current_thread: VALUE = unsafe { rb_funcall(rb_cThread, rb_intern(cstr!("current")), 0) };

        let mut obj = Self::get_struct_from(rbself);
        match &mut obj.session {
            Some(session) => session.mark(current_thread, tag),
            None => panic!("Session is not initialized"),
        }
    }

    // Extract the SessionRubyObject struct from a Ruby object
    unsafe fn get_struct_from(obj: VALUE) -> ManuallyDrop<Box<Self>> {
        unsafe {
            let ptr = rb_check_typeddata(obj, addr_of!(RBDATA));
            ManuallyDrop::new(Box::from_raw(ptr as *mut SessionRubyObject))
        }
    }

    #[allow(non_snake_case)]
    pub unsafe extern "C" fn rb_alloc(_rbself: VALUE) -> VALUE {
        let obj = Box::new(SessionRubyObject { session: None });

        let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));
        let rb_cSession = rb_define_class_under(rb_mPf2, cstr!("Session"), rb_cObject);
        // Wrap the struct into a Ruby object
        rb_data_typed_object_wrap(rb_cSession, Box::into_raw(obj) as *mut c_void, addr_of!(RBDATA))
    }

    unsafe extern "C" fn dmark(ptr: *mut c_void) {
        let obj = ManuallyDrop::new(Box::from_raw(ptr as *mut SessionRubyObject));
        if let Some(session) = &obj.session {
            session.dmark()
        }
    }

    unsafe extern "C" fn dfree(ptr: *mut c_void) {
        drop(Box::from_raw(ptr as *mut SessionRubyObject));
    }

    unsafe extern "C" fn dsize(_: *const c_void) -> size_t {
        // FIXME: Report something better
        mem::size_of::<SessionRubyObject>() as size_t
    }
}

static mut RBDATA: rb_data_type_t = rb_data_type_t {
    wrap_struct_name: cstr!("SessionRubyObject"),
    function: rb_data_type_struct__bindgen_ty_1 {
        dmark: Some(SessionRubyObject::dmark),
        dfree: Some(SessionRubyObject::dfree),
        dsize: Some(SessionRubyObject::dsize),
        dcompact: None,
        reserved: [null_mut(); 1],
    },
    parent: null_mut(),
    data: null_mut(),
    flags: 0,
};
