use std::ffi::CStr;

use rb_sys::*;

use super::profile::{
    Function, FunctionImplementation, FunctionIndex, Location, LocationIndex, Profile, Sample,
};
use crate::util::RTEST;

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

    pub fn serialize(&mut self, source: &crate::profile::Profile) -> String {
        // Fill in meta fields
        self.profile.start_timestamp_ns = source
            .start_timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        self.profile.duration_ns = source
            .end_instant
            .unwrap()
            .duration_since(source.start_instant)
            .as_nanos();

        // Create a Sample for each sample collected
        for sample in source.samples.iter() {
            let mut stack: Vec<LocationIndex> = vec![];

            // Iterate over the Ruby stack
            let ruby_stack_depth = sample.line_count;
            for i in 0..ruby_stack_depth {
                let frame: VALUE = sample.frames[i as usize];
                let lineno: i32 = sample.linenos[i as usize];

                let function = Self::extract_function_from_frame(frame);
                let function_index = self.function_index_for(function);
                let location_index = self.location_index_for(function_index, lineno);

                stack.push(location_index);
            }

            self.profile.samples.push(Sample {
                stack,
                ruby_thread_id: Some(sample.ruby_thread),
            });
        }

        serde_json::to_string(&self.profile).unwrap()
    }

    /// Returns the index of the function in `functions`.
    /// Calling this method will modify `self.profile` in place.
    fn function_index_for(&mut self, function: Function) -> FunctionIndex {
        match self
            .profile
            .functions
            .iter_mut()
            .position(|f| *f == function)
        {
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
        let location = Location {
            function_index,
            lineno,
            address: None,
        };
        match self
            .profile
            .locations
            .iter_mut()
            .position(|l| *l == location)
        {
            Some(index) => index,
            None => {
                self.profile.locations.push(location);
                self.profile.locations.len() - 1
            }
        }
    }

    fn extract_function_from_frame(frame: VALUE) -> Function {
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

            Function {
                implementation: FunctionImplementation::Ruby,
                name: frame_full_label,
                filename: frame_path,
                start_lineno: frame_first_lineno,
                start_address: None,
            }
        }
    }
}
