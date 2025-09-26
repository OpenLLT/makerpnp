use std::mem;
use std::thread::{self, JoinHandle};

use libc::{self, PTHREAD_EXPLICIT_SCHED, SCHED_FIFO, pthread_attr_t, sched_param};

// A generic RT thread that can work with any RT core
pub struct RtThread {}

impl RtThread {
    pub fn spawn<F, T>(priority: u8, f: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        // Set priority (1-99 for RT, higher = more priority)
        assert!(priority >= 1 && priority <= 99);

        // Spawn thread that will configure itself for RT
        let join_handle = thread::spawn(move || {
            // Initialize RT thread attributes
            let mut attr: pthread_attr_t = unsafe { mem::zeroed() };
            let mut param: sched_param = unsafe { mem::zeroed() };

            unsafe {
                // Initialize pthread attributes
                if libc::pthread_attr_init(&mut attr) != 0 {
                    panic!("Failed to initialize pthread attributes");
                }

                // Set scheduling policy to SCHED_FIFO (real-time)
                if libc::pthread_attr_setschedpolicy(&mut attr, SCHED_FIFO) != 0 {
                    panic!("Failed to set scheduling policy");
                }

                param.sched_priority = priority as i32;
                if libc::pthread_attr_setschedparam(&mut attr, &param) != 0 {
                    panic!("Failed to set scheduling parameters");
                }

                // Explicitly set scheduling attributes (don't inherit)
                if libc::pthread_attr_setinheritsched(&mut attr, PTHREAD_EXPLICIT_SCHED) != 0 {
                    panic!("Failed to set explicit scheduling");
                }

                // Apply RT scheduling to current thread
                let thread_id = libc::pthread_self();
                if libc::pthread_setschedparam(thread_id, SCHED_FIFO, &param) != 0 {
                    panic!(
                        "Failed to set thread scheduling parameters: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }

            println!("RT thread starting with priority {}", priority);

            // Call the provided thread entry function with the core pointer
            let result = f();

            result
        });

        join_handle
    }
}
