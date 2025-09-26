use alloc::boxed::Box;
use core::ffi::c_void;

use server_rt_shared::IoStatus;

use crate::SharedState;
use crate::circular_buffer::CircularBuffer;
use crate::rt_time::{get_time_ns, sleep_until_ns};

const LATENCY_BUFFER_SIZE: usize = 100;
const ACCEPTABLE_LATENCY_MICROS: u32 = 50; // +/- 50 microseconds

#[repr(C)]
pub struct Core {
    shared_state: &'static mut SharedState,
    done: bool,
    counter: usize,

    /// Circular buffer to store latency measurements
    /// Note: maximum recorded latency is limited by using a u32 for speed here
    latency_buffer: CircularBuffer<u32, LATENCY_BUFFER_SIZE>,
}

impl Core {
    pub fn new(shared_state_ptr: *mut SharedState) -> Self {
        // Safely access shared state without atomics
        let shared_state = unsafe { &mut *shared_state_ptr };

        Self {
            shared_state,
            done: false,
            counter: 0,
            latency_buffer: CircularBuffer::new(),
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
            let sleep_ns = get_time_ns();
            sleep_until_ns(next_wake_ns);

            let wake_ns = get_time_ns();
            let latency: u64 = wake_ns - sleep_ns;

            // Record the latency in a circular buffer
            self.latency_buffer.push(latency as u32);

            // Calculate the average latency of the entries in the buffer
            let sum = self.latency_buffer.sum();

            // Time-constant division (avoid division by zero)
            let buffer_len = self.latency_buffer.len();
            let avg_latency = if buffer_len > 0 { sum / buffer_len as u32 } else { 0 };

            // Determine if the average latency is acceptable (+/- 50 micro seconds)
            // 1 microsecond = 1000 nanoseconds
            let acceptable_latency_ns = ACCEPTABLE_LATENCY_MICROS * 1000;
            let latency_ok = avg_latency <= acceptable_latency_ns;

            self.shared_state
                .set_stabilized(latency_ok);
        }

        self.counter
    }

    pub fn run(&mut self) {
        // this will be called at a frequency of 1000hz by the `start` method.

        if self
            .shared_state
            .is_shutdown_requested()
        {
            self.done = true;
            return;
        }

        // wait for ready signal from main thread
        if !matches!(self.shared_state.get_io_status(), IoStatus::Ready) {
            return;
        }

        self.process_rt_tasks();
    }

    pub fn process_rt_tasks(&mut self) {
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
