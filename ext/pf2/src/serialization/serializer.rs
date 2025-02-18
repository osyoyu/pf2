use std::ffi::{c_char, CStr, CString};

use rb_sys::*;

use super::profile::{
    Function, FunctionImplementation, FunctionIndex, Location, LocationIndex, Profile, Sample,
};
use crate::backtrace::Backtrace;
use crate::util::{cstr, RTEST};

pub struct ProfileSerializer2 {
    profile: Profile,
}

impl ProfileSerializer2 {
    pub fn new() -> ProfileSerializer2 {
        ProfileSerializer2 {
            profile: Profile {
                start_timestamp_ns: 0,
                duration_ns: 0,
                samples: vec![],
                locations: vec![],
                functions: vec![],
            },
        }
    }

    pub fn serialize(&mut self, source: &crate::profile::Profile) {
        // Fill in meta fields
        self.profile.start_timestamp_ns =
            source.start_timestamp.duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        self.profile.duration_ns =
            source.end_instant.unwrap().duration_since(source.start_instant).as_nanos();

        // Create markers
        let rb_ary: VALUE = unsafe { rb_ary_new_capa(source.markers.len() as i64) };
        for marker in source.markers.iter() {
            unsafe {
                let cstring = CString::new(marker.tag.as_str()).unwrap();
                rb_ary_push(rb_ary, rb_str_new_cstr(cstring.as_ptr()));
            }
        }

        // Create a Sample for each sample collected
        for sample in source.samples.iter() {
            // Iterate over the Ruby stack
            let mut stack: Vec<LocationIndex> = vec![];
            let ruby_stack_depth = sample.line_count;
            for i in 0..ruby_stack_depth {
                let frame: VALUE = sample.frames[i as usize];
                let lineno: i32 = sample.linenos[i as usize];
                let function = Self::extract_function_from_ruby_frame(frame);

                let function_index = self.function_index_for(function);
                let location_index = self.location_index_for(function_index, lineno);
                stack.push(location_index);
            }

            // Iterate over the native stack
            let mut native_stack: Vec<LocationIndex> = vec![];
            let native_stack_depth = sample.c_backtrace_pcs[0];
            for i in 1..(native_stack_depth - 1) {
                let pc = sample.c_backtrace_pcs[i];
                let function = Self::extract_function_from_native_pc(pc, source);

                let function_index = self.function_index_for(function);
                let location_index = self.location_index_for(function_index, 0);
                native_stack.push(location_index);
            }

            self.profile.samples.push(Sample {
                stack,
                native_stack,
                ruby_thread_id: Some(sample.ruby_thread),
                elapsed_ns: sample.timestamp.duration_since(source.start_instant).as_nanos() as u64,
            });
        }
    }

    /// Returns the index of the function in `functions`.
    /// Calling this method will modify `self.profile` in place.
    fn function_index_for(&mut self, function: Function) -> FunctionIndex {
        match self.profile.functions.iter_mut().position(|f| *f == function) {
            Some(index) => index,
            None => {
                self.profile.functions.push(function);
                self.profile.functions.len() - 1
            }
        }
    }

    /// Returns the index of the location in `locations`.
    /// Calling this method will modify `self.profile` in place.
    fn location_index_for(&mut self, function_index: FunctionIndex, lineno: i32) -> LocationIndex {
        // Build a Location based on (1) the Function and (2) the actual line hit during sampling.
        let location = Location { function_index, lineno, address: None };
        match self.profile.locations.iter_mut().position(|l| *l == location) {
            Some(index) => index,
            None => {
                self.profile.locations.push(location);
                self.profile.locations.len() - 1
            }
        }
    }

    /// Build a Function from a Ruby frame.
    fn extract_function_from_ruby_frame(frame: VALUE) -> Function {
        unsafe {
            let mut frame_full_label: VALUE = rb_profile_frame_full_label(frame);
            let frame_full_label: Option<String> = if RTEST(frame_full_label) {
                Some(
                    CStr::from_ptr(rb_string_value_cstr(&mut frame_full_label))
                        .to_str()
                        .unwrap()
                        .to_owned(),
                )
            } else {
                None
            };

            let mut frame_path: VALUE = rb_profile_frame_path(frame);
            let frame_path: Option<String> = if RTEST(frame_path) {
                Some(
                    CStr::from_ptr(rb_string_value_cstr(&mut frame_path))
                        .to_str()
                        .unwrap()
                        .to_owned(),
                )
            } else {
                None
            };

            let frame_first_lineno: VALUE = rb_profile_frame_first_lineno(frame);
            let frame_first_lineno: Option<i32> = if RTEST(frame_first_lineno) {
                Some(rb_num2int(frame_first_lineno).try_into().unwrap())
            } else {
                None
            };

            let start_address = Self::get_underlying_c_function_address(frame);

            Function {
                implementation: FunctionImplementation::Ruby,
                name: frame_full_label,
                filename: frame_path,
                start_lineno: frame_first_lineno,
                start_address,
            }
        }
    }

    fn get_underlying_c_function_address(frame: VALUE) -> Option<usize> {
        unsafe {
            let cme = frame as *mut crate::ruby_internal_apis::rb_callable_method_entry_struct;
            let cme = &*cme; // *mut to reference

            if (*(cme.def)).type_ == 1 {
                // The cme is a Cfunc
                Some((*(cme.def)).cfunc.func as usize)
            } else {
                // The cme is an ISeq (Ruby code) or some other type
                None
            }
        }
    }

    /// Build a Function from a PC (program counter) obtained by libbacktrace.
    fn extract_function_from_native_pc(pc: usize, source: &crate::profile::Profile) -> Function {
        // Obtain the function name and address using libbacktrace
        let mut function: Option<Function> = None;
        Backtrace::backtrace_syminfo(
            &source.backtrace_state,
            pc,
            |_pc: usize, symname: *const c_char, symval: usize, _symsize: usize| unsafe {
                function = Some(Function {
                    implementation: FunctionImplementation::Native,
                    name: if symname.is_null() {
                        None
                    } else {
                        Some(CStr::from_ptr(symname).to_str().unwrap().to_owned())
                    },
                    filename: None,
                    start_lineno: None,
                    start_address: Some(symval),
                });
            },
            Some(Backtrace::backtrace_error_callback),
        );
        function.unwrap()
    }

    pub fn to_ruby_hash(&self) -> VALUE {
        unsafe {
            let hash: VALUE = rb_hash_new();

            // profile[:start_timestamp_ns]
            rb_hash_aset(
                hash,
                rb_id2sym(rb_intern(cstr!("start_timestamp_ns"))),
                rb_int2inum(self.profile.start_timestamp_ns as isize),
            );
            // profile[:duration_ns]
            rb_hash_aset(
                hash,
                rb_id2sym(rb_intern(cstr!("duration_ns"))),
                rb_int2inum(self.profile.duration_ns as isize),
            );

            // profile[:samples]
            let samples: VALUE = rb_ary_new();
            for sample in self.profile.samples.iter() {
                // sample[:stack]
                let stack: VALUE = rb_ary_new();
                for &location_index in sample.stack.iter() {
                    rb_ary_push(stack, rb_int2inum(location_index as isize));
                }
                // sample[:native_stack]
                let native_stack: VALUE = rb_ary_new();
                for &location_index in sample.native_stack.iter() {
                    rb_ary_push(native_stack, rb_int2inum(location_index as isize));
                }
                // sample[:ruby_thread_id]
                let ruby_thread_id = if let Some(ruby_thread_id) = sample.ruby_thread_id {
                    rb_int2inum(ruby_thread_id as isize)
                } else {
                    Qnil as VALUE
                };
                // sample[:elapsed_ns]
                let elapsed_ns = rb_ull2inum(sample.elapsed_ns);

                let sample_hash: VALUE = rb_hash_new();
                rb_hash_aset(sample_hash, rb_id2sym(rb_intern(cstr!("stack"))), stack);
                rb_hash_aset(
                    sample_hash,
                    rb_id2sym(rb_intern(cstr!("native_stack"))),
                    native_stack,
                );
                rb_hash_aset(
                    sample_hash,
                    rb_id2sym(rb_intern(cstr!("ruby_thread_id"))),
                    ruby_thread_id,
                );
                rb_hash_aset(sample_hash, rb_id2sym(rb_intern(cstr!("elapsed_ns"))), elapsed_ns);

                rb_ary_push(samples, sample_hash);
            }
            rb_hash_aset(hash, rb_id2sym(rb_intern(cstr!("samples"))), samples);

            // profile[:locations]
            let locations = rb_ary_new();
            for location in self.profile.locations.iter() {
                let location_hash: VALUE = rb_hash_new();
                // location[:function_index]
                rb_hash_aset(
                    location_hash,
                    rb_id2sym(rb_intern(cstr!("function_index"))),
                    rb_int2inum(location.function_index as isize),
                );
                // location[:lineno]
                rb_hash_aset(
                    location_hash,
                    rb_id2sym(rb_intern(cstr!("lineno"))),
                    rb_int2inum(location.lineno as isize),
                );
                // location[:address]
                rb_hash_aset(
                    location_hash,
                    rb_id2sym(rb_intern(cstr!("address"))),
                    if let Some(address) = location.address {
                        rb_int2inum(address as isize)
                    } else {
                        Qnil as VALUE
                    },
                );
                rb_ary_push(locations, location_hash);
            }
            rb_hash_aset(hash, rb_id2sym(rb_intern(cstr!("locations"))), locations);

            // profile[:functions]
            let functions = rb_ary_new();
            for function in self.profile.functions.iter() {
                let function_hash: VALUE = rb_hash_new();
                // function[:implementation]
                rb_hash_aset(
                    function_hash,
                    rb_id2sym(rb_intern(cstr!("implementation"))),
                    match function.implementation {
                        FunctionImplementation::Ruby => rb_id2sym(rb_intern(cstr!("ruby"))),
                        FunctionImplementation::Native => rb_id2sym(rb_intern(cstr!("native"))),
                    },
                );

                // function[:name]
                let name: VALUE = match &function.name {
                    Some(name) => {
                        let cstring = CString::new(name.as_str()).unwrap();
                        rb_str_new_cstr(cstring.as_ptr())
                    }
                    None => Qnil as VALUE,
                };
                rb_hash_aset(function_hash, rb_id2sym(rb_intern(cstr!("name"))), name);
                // function[:filename]
                let filename: VALUE = match &function.filename {
                    Some(filename) => {
                        let cstring = CString::new(filename.as_str()).unwrap();
                        rb_str_new_cstr(cstring.as_ptr())
                    }
                    None => Qnil as VALUE,
                };
                rb_hash_aset(function_hash, rb_id2sym(rb_intern(cstr!("filename"))), filename);
                // function[:start_lineno]
                rb_hash_aset(
                    function_hash,
                    rb_id2sym(rb_intern(cstr!("start_lineno"))),
                    if let Some(start_lineno) = function.start_lineno {
                        rb_int2inum(start_lineno as isize)
                    } else {
                        Qnil as VALUE
                    },
                );
                // function[:start_address]
                rb_hash_aset(
                    function_hash,
                    rb_id2sym(rb_intern(cstr!("start_address"))),
                    if let Some(start_address) = function.start_address {
                        rb_int2inum(start_address as isize)
                    } else {
                        Qnil as VALUE
                    },
                );
                rb_ary_push(functions, function_hash);
            }
            rb_hash_aset(hash, rb_id2sym(rb_intern(cstr!("functions"))), functions);

            hash
        }
    }
}
