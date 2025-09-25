use alloc::boxed::Box;
use core::ffi::c_void;

use server_rt_shared::IoStatus;

use crate::SharedState;
use crate::rt_time::{get_time_ns, sleep_until_ns};

#[repr(C)]
pub struct Core {
    shared_state: *mut SharedState,
    done: bool,
    counter: usize,
}

impl Core {
    pub fn new(shared_state_ptr: *mut SharedState) -> Self {
        Self {
            shared_state: shared_state_ptr,
            done: false,
            counter: 0,
        }
    }

    pub fn start(&mut self) -> usize {
        // this will be called once, when the rt thread is started

        // Calculate initial wake time
        let mut next_wake_ns = get_time_ns();
        let period_ns: u64 = 1_000_000; // 1ms in nanoseconds

        // Main real-time loop
        loop {
            self.counter += 1;
            // Run core processing
            self.run();

            if self.done {
                break;
            }

            // Calculate next wake time
            next_wake_ns += period_ns;

            // Sleep until next wake time using clock_nanosleep with TIMER_ABSTIME
            // for deterministic timing
            sleep_until_ns(next_wake_ns);
        }

        self.counter
    }

    pub fn run(&mut self) {
        // this will be called at a frequency of 1000hz by the `start` method.

        // Safely access shared state without atomics
        let shared_state = unsafe { &mut *self.shared_state };

        // wait for ready signal from main thread
        if !matches!(shared_state.get_io_status(), IoStatus::Ready) {
            return;
        }

        self.process_rt_tasks(shared_state);
    }

    pub fn process_rt_tasks(&mut self, _shared_state: &mut SharedState) {
        if self.counter == 5000 {
            self.done = true;
        }
    }
}

pub mod core_ffi {
    use super::*;
    // Create a new Core instance and return opaque pointer for rt_thread_entry
    #[unsafe(no_mangle)]
    pub extern "C" fn create_core_for_thread(shared_state_ptr: *mut c_void) -> *mut c_void {
        let core = Box::new(Core::new(shared_state_ptr as *mut crate::SharedState));
        Box::into_raw(core) as *mut c_void
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rt_thread_entry(data_ptr: *mut c_void) -> usize {
        // This function receives an opaque pointer and manages the real-time loop
        // The pointer should be a properly initialized Core instance
        let core_ptr = data_ptr as *mut Core;
        let core = unsafe { &mut *core_ptr };

        core.start()
    }
}
