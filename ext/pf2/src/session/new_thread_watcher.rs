use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::Mutex;

use rb_sys::*;

/// A helper to watch new Ruby threads.
///
/// `NewThreadWatcher` operates on the Events Hooks API.
/// Instead of relying on the `THREAD_EVENT_STARTED` event, it combines the
/// `THREAD_EVENT_RESUMED` event and an internal _known-threads_ record.
///
/// This is to support operations requiring the underlying pthread. Ruby Threads
/// are not guaranteed to be fully initialized at the time
/// `THREAD_EVENT_STARTED` is triggered; i.e. the underlying pthread has not
/// been created yet and `Thread#native_thread_id` returns `nil`.
pub struct NewThreadWatcher {
    inner: Rc<Mutex<Inner>>,
    event_hook: *mut rb_internal_thread_event_hook_t,
}

struct Inner {
    known_threads: HashSet<VALUE>,
    on_new_thread: Box<dyn Fn(VALUE)>,
}

impl NewThreadWatcher {
    pub fn watch<F>(callback: F) -> Self
    where
        F: Fn(VALUE) + 'static,
    {
        let mut watcher = Self {
            inner: Rc::new(Mutex::new(Inner {
                known_threads: HashSet::new(),
                on_new_thread: Box::new(callback),
            })),
            event_hook: null_mut(),
        };

        let inner_ptr = Rc::into_raw(Rc::clone(&watcher.inner));
        unsafe {
            watcher.event_hook = rb_internal_thread_add_event_hook(
                Some(Self::on_thread_resume),
                RUBY_INTERNAL_THREAD_EVENT_RESUMED,
                inner_ptr as *mut c_void,
            );
        };

        watcher
    }

    unsafe extern "C" fn on_thread_resume(
        _flag: rb_event_flag_t,
        data: *const rb_internal_thread_event_data_t,
        custom_data: *mut c_void,
    ) {
        let ruby_thread: VALUE = unsafe { (*data).thread };

        // A pointer to Box<Inner> is passed as custom_data
        let inner = unsafe { ManuallyDrop::new(Box::from_raw(custom_data as *mut Mutex<Inner>)) };
        let mut inner = inner.lock().unwrap();

        if !inner.known_threads.contains(&ruby_thread) {
            inner.known_threads.insert(ruby_thread);
            (inner.on_new_thread)(ruby_thread);
        }
    }
}

impl Drop for NewThreadWatcher {
    fn drop(&mut self) {
        log::trace!("Cleaning up event hook");
        unsafe {
            rb_internal_thread_remove_event_hook(self.event_hook);
        }
    }
}
