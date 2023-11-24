#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_int, c_void, CString};
use std::mem;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use rb_sys::*;

use crate::profile::Profile;
use crate::sample_collector::{Sample, SampleFrame};
use crate::util::*;

static mut RBDATA: rb_data_type_t = rb_data_type_t {
    wrap_struct_name: cstr!("TimerCollectorInternal"),
    function: rb_data_type_struct__bindgen_ty_1 {
        dmark: Some(TimerCollector::dmark),
        dfree: Some(TimerCollector::dfree),
        dsize: Some(TimerCollector::dsize),
        dcompact: None,
        reserved: [null_mut(); 1],
    },
    parent: null_mut(),
    data: null_mut(),
    flags: 0,
};

#[derive(Clone, Debug)]
pub struct TimerCollector {
    start_time: Instant,
    ruby_threads: Arc<RwLock<Vec<VALUE>>>,
    samples: Arc<RwLock<Vec<Sample>>>,
    stop_requested: Arc<AtomicBool>,
    signal_handler_thread_tid: Arc<AtomicI64>,
}

#[derive(Debug)]
struct CollectorThreadData {
    start_time: Instant,
    ruby_thread: VALUE,
    samples: Arc<RwLock<Vec<Sample>>>,
}

impl TimerCollector {
    fn new() -> Self {
        TimerCollector {
            start_time: Instant::now(),
            ruby_threads: Arc::new(RwLock::new(vec![])),
            samples: Arc::new(RwLock::new(vec![])),
            stop_requested: Arc::new(AtomicBool::new(false)),
            signal_handler_thread_tid: Arc::new(AtomicI64::new(0)), // TODO: Option
        }
    }

    fn start(&mut self, _rbself: VALUE, ruby_threads: VALUE) -> VALUE {
        // Register threads
        {
            let stored_threads = &mut self.ruby_threads.try_write().unwrap();
            unsafe {
                for i in 0..RARRAY_LEN(ruby_threads) {
                    stored_threads.push(rb_ary_entry(ruby_threads, i));
                }
            }
        }

        self.create_signal_handler_thread();
        thread::sleep(Duration::from_millis(10));

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

    fn install_to_current_thread(&self, _rbself: VALUE) -> VALUE {
        let ruby_thread: VALUE = unsafe { rb_thread_current() };
        let data_for_job: Arc<CollectorThreadData> = Arc::new(CollectorThreadData {
            start_time: self.start_time,
            ruby_thread,
            samples: Arc::clone(&self.samples),
        });

        // Wanted to use SIGEV_THREAD, but it's not exposed through the libc crate
        let mut timer_id: mem::MaybeUninit<libc::timer_t> = mem::MaybeUninit::uninit();
        let mut sigevent: libc::sigevent = unsafe { mem::zeroed() };
        sigevent.sigev_notify = libc::SIGEV_THREAD_ID;
        sigevent.sigev_signo = libc::SIGALRM;
        sigevent.sigev_value.sival_ptr = Arc::into_raw(Arc::clone(&data_for_job)) as *mut c_void;
        sigevent.sigev_notify_thread_id = self
            .signal_handler_thread_tid
            .load(Ordering::Relaxed)
            .try_into()
            .unwrap();
        unsafe {
            let err = libc::timer_create(
                libc::CLOCK_THREAD_CPUTIME_ID,
                &mut sigevent,
                timer_id.as_mut_ptr(),
            );
            if err != 0 {
                panic!("timer_create failed: {}", err);
            }
        }

        // Configure timer to fire every 50 ms of CPU time
        let mut its: libc::itimerspec = unsafe { mem::zeroed() };
        its.it_interval.tv_sec = 0;
        its.it_interval.tv_nsec = 10_000_000; // 10 ms
        its.it_value.tv_sec = 0;
        its.it_value.tv_nsec = 10_000_000; // 10 ms
        unsafe {
            let err = libc::timer_settime(*timer_id.as_ptr(), 0, &its, null_mut());
            if err != 0 {
                panic!("timer_settime failed: {}", err);
            }
        }

        unsafe {
            println!("Installed to tid={}", libc::syscall(libc::SYS_gettid));
        }
        Qtrue.into()
    }

    fn create_signal_handler_thread(&self) {
        let tid = Arc::clone(&self.signal_handler_thread_tid);
        thread::spawn(move || {
            tid.store(
                unsafe { libc::syscall(libc::SYS_gettid) },
                Ordering::Relaxed,
            );

            // Install sigaction to this thread
            let mut sigaction: libc::sigaction = unsafe { mem::zeroed() };
            sigaction.sa_sigaction = Self::signal_handler as usize;
            sigaction.sa_flags = libc::SA_SIGINFO;
            unsafe {
                let err = libc::sigaction(libc::SIGALRM, &sigaction, null_mut());
                if err != 0 {
                    panic!("sigaction failed: {}", err);
                }
            }

            loop {
                // do nothing; wait for signal
                thread::sleep(Duration::from_millis(1));
            }
        });
    }

    extern "C" fn signal_handler(_sig: c_int, info: *mut c_void, _ucontext: *mut c_void) {
        // println!("signal!");
        let ptr = unsafe { extract_si_value_sival_ptr(info) as *mut CollectorThreadData };
        let data = unsafe { Arc::from_raw(ptr) };

        unsafe {
            rb_postponed_job_register_one(
                0,
                Some(Self::postponed_job),
                Arc::into_raw(data) as *mut c_void,
            );
        }
    }

    unsafe extern "C" fn postponed_job(data: *mut c_void) {
        let data = unsafe { Arc::from_raw(data as *mut CollectorThreadData) };

        // Collect stack information from specified Ruby Threads
        let mut samples_to_push: Vec<Sample> = vec![];
        if unsafe { rb_funcall(data.ruby_thread, rb_intern(cstr!("status")), 0) } == Qfalse as u64 {
            return;
        }

        let mut buffer: [VALUE; 2000] = [0; 2000];
        let mut linebuffer: [i32; 2000] = [0; 2000];
        let lines: c_int = unsafe {
            rb_profile_thread_frames(
                data.ruby_thread,
                0,
                2000,
                buffer.as_mut_ptr(),
                linebuffer.as_mut_ptr(),
            )
        };

        // FIXME: Will this really occur?
        if lines == 0 {
            return;
        }

        let mut sample = Sample {
            elapsed_ns: Instant::now().duration_since(data.start_time).as_nanos(),
            ruby_thread: data.ruby_thread,
            ruby_thread_native_thread_id: unsafe {
                rb_num2int(rb_funcall(
                    data.ruby_thread,
                    rb_intern(cstr!("native_thread_id")),
                    0,
                ))
            },
            frames: vec![],
        };
        for i in 0..lines {
            let iseq: VALUE = buffer[i as usize];
            let lineno: i32 = linebuffer[i as usize];
            sample.frames.push(SampleFrame { iseq, lineno });
        }
        samples_to_push.push(sample);

        // Try to lock samples; if failed, just skip this sample
        // (dmark (GC) might be locking data.samples)
        if let Ok(mut samples) = data.samples.try_write() {
            samples.append(&mut samples_to_push);
        } else {
            println!("Failed to record samples (could not acquire lock on samples)")
        };

        mem::forget(data); // FIXME: something's leaking
    }

    // ----------

    // Obtain the Ruby VALUE of `Pf2::TimerCollector`.
    #[allow(non_snake_case)]
    fn get_ruby_class() -> VALUE {
        unsafe {
            let rb_mPf2: VALUE = rb_define_module(cstr!("Pf2"));
            rb_define_class_under(rb_mPf2, cstr!("TimerCollector"), rb_cObject)
        }
    }

    fn get_struct_from(obj: VALUE) -> Box<Self> {
        unsafe { Box::from_raw(rb_check_typeddata(obj, &RBDATA) as *mut TimerCollector) }
    }

    fn wrap_struct(collector: TimerCollector) -> VALUE {
        #[allow(non_snake_case)]
        let rb_cTimerCollector = Self::get_ruby_class();

        unsafe {
            rb_data_typed_object_wrap(
                rb_cTimerCollector,
                Box::into_raw(Box::new(collector)) as *mut _ as *mut c_void,
                &RBDATA,
            )
        }
    }

    unsafe extern "C" fn dmark(ptr: *mut c_void) {
        unsafe {
            let collector: Box<TimerCollector> = Box::from_raw(ptr as *mut TimerCollector);

            // Mark collected sample VALUEs
            {
                let samples = collector.samples.try_read().unwrap();
                for sample in samples.iter() {
                    rb_gc_mark(sample.ruby_thread);
                    for frame in sample.frames.iter() {
                        rb_gc_mark(frame.iseq)
                    }
                }
            }

            mem::forget(collector);
        }
    }
    unsafe extern "C" fn dfree(ptr: *mut c_void) {
        unsafe {
            let collector: Box<TimerCollector> = Box::from_raw(ptr as *mut TimerCollector);
            drop(collector);
        }
    }
    unsafe extern "C" fn dsize(_: *const c_void) -> size_t {
        // FIXME: Report something better
        mem::size_of::<TimerCollector>() as size_t
    }

    pub unsafe extern "C" fn rb_alloc(_rbself: VALUE) -> VALUE {
        let collector = TimerCollector::new();
        Self::wrap_struct(collector)
    }

    // TimerCollector.start
    pub unsafe extern "C" fn rb_start(rbself: VALUE, ruby_threads: VALUE) -> VALUE {
        let mut collector = Self::get_struct_from(rbself);
        let ret = collector.start(rbself, ruby_threads);
        mem::forget(collector);
        ret
    }

    // TimerCollector.stop
    pub unsafe extern "C" fn rb_stop(rbself: VALUE) -> VALUE {
        let collector = Self::get_struct_from(rbself);
        let ret = collector.stop(rbself);
        mem::forget(collector);
        ret
    }

    // TimerCollector.install_to_current_thread
    pub unsafe extern "C" fn rb_install_to_current_thread(rbself: VALUE) -> VALUE {
        let collector = Self::get_struct_from(rbself);
        let ret = collector.install_to_current_thread(rbself);
        mem::forget(collector);
        ret
    }
}
