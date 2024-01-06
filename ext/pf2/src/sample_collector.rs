#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_int, c_void, CString};
use std::mem;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use rb_sys::*;

use crate::profile::Profile;
use crate::util::*;

unsafe extern "C" fn dmark(ptr: *mut c_void) {
    unsafe {
        let collector: Box<SampleCollector> = Box::from_raw(ptr as *mut SampleCollector);

        // Mark collected sample VALUEs
        {
            let samples = collector.samples.try_read().unwrap();
            for sample in samples.iter() {
                rb_gc_mark(sample.ruby_thread);
                for frame in sample.frames.iter() {
                    rb_gc_mark(*frame);
                }
            }
        }

        mem::forget(collector);
    }
}
unsafe extern "C" fn dfree(ptr: *mut c_void) {
    unsafe {
        let collector: Box<SampleCollector> = Box::from_raw(ptr as *mut SampleCollector);
        drop(collector);
    }
}
unsafe extern "C" fn dsize(_: *const c_void) -> size_t {
    // FIXME: Report something better
    mem::size_of::<SampleCollector>() as size_t
}

static mut RBDATA: rb_data_type_t = rb_data_type_t {
    wrap_struct_name: cstr!("SampleCollectorInternal"),
    function: rb_data_type_struct__bindgen_ty_1 {
        dmark: Some(dmark),
        dfree: Some(dfree),
        dsize: Some(dsize),
        dcompact: None,
        reserved: [null_mut(); 1],
    },
    parent: null_mut(),
    data: null_mut(),
    flags: 0,
};

// TODO: To be renamed to TimerThreadScheduler
#[derive(Clone, Debug)]
pub struct SampleCollector {
    start_time: Instant,
    ruby_threads: Arc<RwLock<Vec<VALUE>>>,
    samples: Arc<RwLock<Vec<Sample>>>,
    stop_requested: Arc<AtomicBool>,
}

#[derive(Debug)]
struct CollectorThreadData {
    start_time: Instant,
    ruby_threads: Arc<RwLock<Vec<VALUE>>>,
    samples: Arc<RwLock<Vec<Sample>>>,
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub elapsed_ns: u128,
    pub ruby_thread: VALUE,
    pub ruby_thread_native_thread_id: i64,
    pub frames: Vec<VALUE>,
}

impl SampleCollector {
    fn new() -> Self {
        SampleCollector {
            start_time: Instant::now(),
            ruby_threads: Arc::new(RwLock::new(vec![])),
            samples: Arc::new(RwLock::new(vec![])),
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

        // Start monitoring thread
        let stop_requested = Arc::clone(&self.stop_requested);
        let data_for_job: Arc<CollectorThreadData> = Arc::new(CollectorThreadData {
            start_time: self.start_time,
            ruby_threads: Arc::clone(&self.ruby_threads),
            samples: Arc::clone(&self.samples),
        });
        thread::spawn(move || Self::thread_main_loop(stop_requested, data_for_job));

        Qtrue.into()
    }

    fn stop(&self, _rbself: VALUE) -> VALUE {
        // Stop the collector thread
        self.stop_requested.store(true, Ordering::Relaxed);
        let profile = Profile::from_samples(&self.samples.try_read().unwrap());

        let json = serde_json::to_string(&profile).unwrap();
        let json_cstring = CString::new(json).unwrap();
        unsafe { rb_str_new_cstr(json_cstring.as_ptr()) }
    }

    fn thread_main_loop(stop_requested: Arc<AtomicBool>, data_for_job: Arc<CollectorThreadData>) {
        loop {
            if stop_requested.fetch_and(true, Ordering::Relaxed) {
                break;
            }

            let data_for_job = Arc::clone(&data_for_job);
            unsafe {
                // FIXME: data_for_job has a high chance of leaking memory here,
                // as rb_postponed_job_register_one does not invoke postponed_job().
                rb_postponed_job_register_one(
                    0,
                    Some(Self::postponed_job),
                    Arc::into_raw(data_for_job) as *mut c_void,
                );

                // sleep for 50 ms
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    unsafe extern "C" fn postponed_job(ptr: *mut c_void) {
        let data = unsafe { Arc::from_raw(ptr as *mut CollectorThreadData) };
        // Collect stack information from specified Ruby Threads
        let mut samples_to_push: Vec<Sample> = vec![];
        let ruby_threads = data.ruby_threads.try_read().unwrap();
        for ruby_thread in ruby_threads.iter() {
            if unsafe { rb_funcall(*ruby_thread, rb_intern(cstr!("status")), 0) } == Qfalse as u64 {
                continue;
            }

            let mut buffer: [VALUE; 2000] = [0; 2000];
            let mut linebuffer: [i32; 2000] = [0; 2000];

            let lines: c_int = unsafe {
                rb_profile_thread_frames(
                    *ruby_thread,
                    0,
                    2000,
                    buffer.as_mut_ptr(),
                    linebuffer.as_mut_ptr(),
                )
            };

            // FIXME: Will this really occur?
            if lines == 0 {
                continue;
            }

            let mut sample = Sample {
                elapsed_ns: Instant::now().duration_since(data.start_time).as_nanos(),
                ruby_thread: *ruby_thread,
                ruby_thread_native_thread_id: unsafe {
                    rb_num2int(rb_funcall(
                        *ruby_thread,
                        rb_intern(cstr!("native_thread_id")),
                        0,
                    ))
                },
                frames: vec![],
            };
            for i in 0..lines {
                let frame: VALUE = buffer[i as usize];
                sample.frames.push(frame);
            }
            samples_to_push.push(sample);
        }

        // Try to lock samples; if failed, just skip this sample
        // (dmark (GC) might be locking data.samples)
        if let Ok(mut samples) = data.samples.try_write() {
            samples.append(&mut samples_to_push);
        } else {
            println!("Failed to record samples (could not acquire lock on samples)")
        };
    }

    // -----

    // Obtain the Ruby VALUE of `Pf2::SampleCollector`.
    #[allow(non_snake_case)]
    fn get_ruby_class() -> VALUE {
        unsafe {
            let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));
            rb_define_class_under(rb_mPf2, cstr!("SampleCollector"), rb_cObject)
        }
    }

    fn get_struct_from(obj: VALUE) -> Box<Self> {
        unsafe { Box::from_raw(rb_check_typeddata(obj, &RBDATA) as *mut SampleCollector) }
    }

    fn wrap_struct(collector: SampleCollector) -> VALUE {
        #[allow(non_snake_case)]
        let rb_cSampleCollector = Self::get_ruby_class();

        unsafe {
            rb_data_typed_object_wrap(
                rb_cSampleCollector,
                Box::into_raw(Box::new(collector)) as *mut _ as *mut c_void,
                &RBDATA,
            )
        }
    }

    pub unsafe extern "C" fn rb_alloc(_rbself: VALUE) -> VALUE {
        let collector = SampleCollector::new();
        Self::wrap_struct(collector)
    }

    // SampleCollector.start
    pub unsafe extern "C" fn rb_start(rbself: VALUE, ruby_threads: VALUE) -> VALUE {
        let mut collector = Self::get_struct_from(rbself);
        let ret = collector.start(rbself, ruby_threads);
        mem::forget(collector);
        ret
    }

    // SampleCollector.stop
    pub unsafe extern "C" fn rb_stop(rbself: VALUE) -> VALUE {
        let collector = Self::get_struct_from(rbself);
        let ret = collector.stop(rbself);
        mem::forget(collector);
        ret
    }
}
