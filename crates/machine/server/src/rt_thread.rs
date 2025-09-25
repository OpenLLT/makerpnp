use std::mem;
use std::ptr;
use std::thread::{self, JoinHandle};

use libc::{
    self, CLOCK_MONOTONIC, EINTR, PTHREAD_EXPLICIT_SCHED, SCHED_FIFO, TIMER_ABSTIME, pthread_attr_t, sched_param,
    timespec,
};
use server_rt_core::{SharedState, rt_ffi};
use server_rt_shared::sendable_ptr::SendablePtr;

pub struct RtThread {
    thread_handle: Option<JoinHandle<()>>,
    priority: i32,
}

// Based on the C++ Thread class in the reference article
impl RtThread {
    pub fn new(priority: i32) -> Self {
        Self {
            thread_handle: None,
            priority,
        }
    }

    pub fn start(&mut self, shared_state_ptr: *mut SharedState) {
        // First ensure memory is locked to prevent paging
        unsafe {
            if libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) != 0 {
                eprintln!(
                    "Failed to lock memory with mlockall: {}",
                    std::io::Error::last_os_error()
                );
            }
        }

        // Create a SendablePtr to allow the pointer to cross thread boundaries
        let sendable_ptr = SendablePtr::new(shared_state_ptr);
        let priority = self.priority;

        // Spawn thread that will configure itself for RT
        self.thread_handle = Some(thread::spawn(move || {
            // Get raw pointer back
            let shared_state_ptr = sendable_ptr.get();

            // Initialize RT thread attributes
            let mut attr: pthread_attr_t = unsafe { mem::zeroed() };
            let mut param: sched_param = unsafe { mem::zeroed() };

            unsafe {
                // Initialize pthread attributes
                if libc::pthread_attr_init(&mut attr) != 0 {
                    eprintln!("Failed to initialize pthread attributes");
                    return;
                }

                // Set scheduling policy to SCHED_FIFO (real-time)
                if libc::pthread_attr_setschedpolicy(&mut attr, SCHED_FIFO) != 0 {
                    eprintln!("Failed to set scheduling policy");
                    return;
                }

                // Set priority (1-99 for RT, higher = more priority)
                param.sched_priority = priority;
                if libc::pthread_attr_setschedparam(&mut attr, &param) != 0 {
                    eprintln!("Failed to set scheduling parameters");
                    return;
                }

                // Explicitly set scheduling attributes (don't inherit)
                if libc::pthread_attr_setinheritsched(&mut attr, PTHREAD_EXPLICIT_SCHED) != 0 {
                    eprintln!("Failed to set explicit scheduling");
                    return;
                }

                // Apply RT scheduling to current thread
                let thread_id = libc::pthread_self();
                if libc::pthread_setschedparam(thread_id, SCHED_FIFO, &param) != 0 {
                    eprintln!(
                        "Failed to set thread scheduling parameters: {}",
                        std::io::Error::last_os_error()
                    );
                    // Continue anyway, as we may still be able to run with reduced capabilities
                }
            }

            println!("RT thread starting with priority {}", priority);

            // Create RT core instance and initialize it
            let core_ptr = rt_ffi::core_new(shared_state_ptr);
            unsafe { rt_ffi::core_start(core_ptr) };

            // Calculate initial wake time
            let mut next_wake_ns = Self::get_time_ns();
            let period_ns: u64 = 1_000_000; // 1ms in nanoseconds

            // Main real-time loop
            loop {
                // Run core processing
                unsafe { rt_ffi::core_run(core_ptr) };

                // Calculate next wake time
                next_wake_ns += period_ns;

                // Sleep until next wake time using clock_nanosleep with TIMER_ABSTIME
                // for deterministic timing
                Self::sleep_until_ns(next_wake_ns);
            }
        }));
    }

    // Get current time in nanoseconds using CLOCK_MONOTONIC
    fn get_time_ns() -> u64 {
        let mut ts: timespec = unsafe { mem::zeroed() };
        unsafe {
            libc::clock_gettime(CLOCK_MONOTONIC, &mut ts);
        }
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }

    // Sleep until specific time using high-precision clock_nanosleep
    fn sleep_until_ns(target_ns: u64) {
        let sec = (target_ns / 1_000_000_000) as i64;
        let nsec = (target_ns % 1_000_000_000) as i64;

        let ts = timespec {
            tv_sec: sec,
            tv_nsec: nsec,
        };

        // Use TIMER_ABSTIME for absolute timing (not relative)
        unsafe {
            while libc::clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &ts, ptr::null_mut()) == EINTR {
                // If interrupted by signal, retry
            }
        }
    }
}
