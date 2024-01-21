#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_void, CString};
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use rb_sys::*;

use crate::profile::Profile;
use crate::profile_serializer::ProfileSerializer;
use crate::sample::Sample;
use crate::util::*;

#[derive(Clone, Debug)]
pub struct TimerThreadScheduler {
    ruby_threads: Arc<RwLock<Vec<VALUE>>>,
    profile: Option<Arc<RwLock<Profile>>>,
    stop_requested: Arc<AtomicBool>,
}

#[derive(Debug)]
struct PostponedJobArgs {
    ruby_threads: Arc<RwLock<Vec<VALUE>>>,
    profile: Arc<RwLock<Profile>>,
}

impl TimerThreadScheduler {
    fn new() -> Self {
        TimerThreadScheduler {
            ruby_threads: Arc::new(RwLock::new(vec![])),
            profile: None,
            stop_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start(&mut self, _rbself: VALUE, ruby_threads: VALUE) -> VALUE {
        // Register threads
        let stored_threads = &mut self.ruby_threads.try_write().unwrap();
        unsafe {
            for i in 0..RARRAY_LEN(ruby_threads) {
                stored_threads.push(rb_ary_entry(ruby_threads, i));
            }
        }

        // Create Profile
        let profile = Arc::new(RwLock::new(Profile::new()));
        self.start_profile_buffer_flusher_thread(&profile);

        // Start monitoring thread
        let stop_requested = Arc::clone(&self.stop_requested);
        let postponed_job_args: Box<PostponedJobArgs> = Box::new(PostponedJobArgs {
            ruby_threads: Arc::clone(&self.ruby_threads),
            profile: Arc::clone(&profile),
        });
        let postponed_job_handle: rb_postponed_job_handle_t = unsafe {
            rb_postponed_job_preregister(
                0,
                Some(Self::postponed_job),
                Box::into_raw(postponed_job_args) as *mut c_void, // FIXME: leak
            )
        };
        thread::spawn(move || Self::thread_main_loop(stop_requested, postponed_job_handle));

        self.profile = Some(profile);

        Qtrue.into()
    }

    fn thread_main_loop(
        stop_requested: Arc<AtomicBool>,
        postponed_job_handle: rb_postponed_job_handle_t,
    ) {
        loop {
            if stop_requested.fetch_and(true, Ordering::Relaxed) {
                break;
            }
            unsafe {
                rb_postponed_job_trigger(postponed_job_handle);
            }
            // sleep for 50 ms
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn stop(&self, _rbself: VALUE) -> VALUE {
        // Stop the collector thread
        self.stop_requested.store(true, Ordering::Relaxed);

        if let Some(profile) = &self.profile {
            // Finalize
            match profile.try_write() {
                Ok(mut profile) => {
                    profile.flush_temporary_sample_buffer();
                }
                Err(_) => {
                    println!("[pf2 ERROR] stop: Failed to acquire profile lock.");
                    return Qfalse.into();
                }
            }

            let profile = profile.try_read().unwrap();
            log::debug!("Number of samples: {}", profile.samples.len());

            let serialized = ProfileSerializer::serialize(&profile);
            let serialized = CString::new(serialized).unwrap();
            unsafe { rb_str_new_cstr(serialized.as_ptr()) }
        } else {
            panic!("stop() called before start()");
        }
    }

    unsafe extern "C" fn postponed_job(ptr: *mut c_void) {
        unsafe {
            rb_gc_disable();
        }
        let args = unsafe { ManuallyDrop::new(Box::from_raw(ptr as *mut PostponedJobArgs)) };

        let mut profile = match args.profile.try_write() {
            Ok(profile) => profile,
            Err(_) => {
                // FIXME: Do we want to properly collect GC samples? I don't know yet.
                log::trace!("Failed to acquire profile lock (garbage collection possibly in progress). Dropping sample.");
                return;
            }
        };

        // Collect stack information from specified Ruby Threads
        let ruby_threads = args.ruby_threads.try_read().unwrap();
        for ruby_thread in ruby_threads.iter() {
            // Check if the thread is still alive
            if unsafe { rb_funcall(*ruby_thread, rb_intern(cstr!("status")), 0) } == Qfalse as u64 {
                continue;
            }

            let sample = Sample::capture(*ruby_thread);
            if profile.temporary_sample_buffer.push(sample).is_err() {
                log::debug!("Temporary sample buffer full. Dropping sample.");
            }
        }
        unsafe {
            rb_gc_enable();
        }
    }

    fn start_profile_buffer_flusher_thread(&self, profile: &Arc<RwLock<Profile>>) {
        let profile = Arc::clone(profile);
        thread::spawn(move || loop {
            log::trace!("Flushing temporary sample buffer");
            match profile.try_write() {
                Ok(mut profile) => {
                    profile.flush_temporary_sample_buffer();
                }
                Err(_) => {
                    log::debug!("flusher: Failed to acquire profile lock");
                }
            }
            thread::sleep(Duration::from_millis(500));
        });
    }

    // Ruby Methods

    // SampleCollector.start
    pub unsafe extern "C" fn rb_start(rbself: VALUE, ruby_threads: VALUE, _: VALUE) -> VALUE {
        let mut collector = Self::get_struct_from(rbself);
        collector.start(rbself, ruby_threads)
    }

    // SampleCollector.stop
    pub unsafe extern "C" fn rb_stop(rbself: VALUE) -> VALUE {
        let collector = Self::get_struct_from(rbself);
        collector.stop(rbself)
    }

    // Functions for TypedData

    fn get_struct_from(obj: VALUE) -> ManuallyDrop<Box<Self>> {
        unsafe {
            let ptr = rb_check_typeddata(obj, &RBDATA);
            ManuallyDrop::new(Box::from_raw(ptr as *mut TimerThreadScheduler))
        }
    }

    #[allow(non_snake_case)]
    pub unsafe extern "C" fn rb_alloc(_rbself: VALUE) -> VALUE {
        let collector = TimerThreadScheduler::new();

        unsafe {
            let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));
            let rb_cTimerThreadScheduler =
                rb_define_class_under(rb_mPf2, cstr!("TimerThreadScheduler"), rb_cObject);

            rb_data_typed_object_wrap(
                rb_cTimerThreadScheduler,
                Box::into_raw(Box::new(collector)) as *mut _ as *mut c_void,
                &RBDATA,
            )
        }
    }

    unsafe extern "C" fn dmark(ptr: *mut c_void) {
        unsafe {
            let collector = ManuallyDrop::new(Box::from_raw(ptr as *mut TimerThreadScheduler));
            if let Some(profile) = &collector.profile {
                match profile.read() {
                    Ok(profile) => {
                        profile.dmark();
                    }
                    Err(_) => {
                        panic!("[pf2 FATAL] dmark: Failed to acquire profile lock.");
                    }
                }
            }
        }
    }
    unsafe extern "C" fn dfree(ptr: *mut c_void) {
        unsafe {
            drop(Box::from_raw(ptr as *mut TimerThreadScheduler));
        }
    }
    unsafe extern "C" fn dsize(_: *const c_void) -> size_t {
        // FIXME: Report something better
        std::mem::size_of::<TimerThreadScheduler>() as size_t
    }
}

static mut RBDATA: rb_data_type_t = rb_data_type_t {
    wrap_struct_name: cstr!("TimerThreadScheduler"),
    function: rb_data_type_struct__bindgen_ty_1 {
        dmark: Some(TimerThreadScheduler::dmark),
        dfree: Some(TimerThreadScheduler::dfree),
        dsize: Some(TimerThreadScheduler::dsize),
        dcompact: None,
        reserved: [null_mut(); 1],
    },
    parent: null_mut(),
    data: null_mut(),
    flags: 0,
};
