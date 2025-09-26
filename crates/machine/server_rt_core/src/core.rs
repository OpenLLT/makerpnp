use alloc::boxed::Box;
use core::ffi::c_void;

use server_rt_shared::IoStatus;

use crate::SharedState;
use crate::circular_buffer::CircularBuffer;
use crate::rt_time::{get_time_ns, sleep_until_ns};

const LATENCY_BUFFER_SIZE: usize = 100;
/// 50 microseconds = 50_000 nanoseconds
const ACCEPTABLE_DEVIATION_NS: u32 = 200_000;
const LATENCY_DEVIATION_THRESHOLD_PERCENTAGE: u8 = 95;

#[repr(C)]
pub struct Core {
    shared_state: &'static mut SharedState,
    done: bool,
    counter: usize,

    /// Circular buffer to store latency measurements
    /// Note: maximum recorded latency is limited by using a u32 for speed here
    latency_buffer: CircularBuffer<i32, LATENCY_BUFFER_SIZE>,
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
            sleep_until_ns(next_wake_ns);
            let wake_ns = get_time_ns();

            self.determine_stability(
                next_wake_ns,
                wake_ns,
                LATENCY_DEVIATION_THRESHOLD_PERCENTAGE,
                ACCEPTABLE_DEVIATION_NS,
            );
            self.update_latency_stats();
        }

        self.counter
    }

    fn update_latency_stats(&mut self) {
        let mut latency_stats = [0; 100];
        self.latency_buffer
            .iter()
            .enumerate()
            .for_each(|(i, latency)| {
                latency_stats[i] = *latency;
            });

        self.shared_state
            .set_latency_stats(latency_stats)
    }

    /// System is considered stable when a percentage of the latency measurements are within the acceptable range
    fn determine_stability(
        &mut self,
        next_wake_ns: u64,
        wake_ns: u64,
        threshold_percentage: u8,
        acceptable_deviation: u32,
    ) {
        // Calculate latency deviation (signed value)
        // This represents how far off we are from the target wakeup time
        let latency_deviation = wake_ns as i64 - next_wake_ns as i64;

        // Convert to i32 for storage in the buffer (safe for reasonable latencies)
        let latency_deviation_i32 = latency_deviation as i32;

        // Record the latency deviation in the circular buffer
        self.latency_buffer
            .push(latency_deviation_i32);

        // Calculate statistics for acceptability check
        let buffer_len = self.latency_buffer.len();

        // Check if each individual latency value is within acceptable range
        let acceptable_entries = self
            .latency_buffer
            .iter()
            .filter(|&&x| x.abs() <= acceptable_deviation as i32)
            .count();

        let stability_threshold = (buffer_len * threshold_percentage as usize) / 100;
        let latency_ok = acceptable_entries >= stability_threshold;

        self.shared_state
            .set_stabilized(latency_ok);
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
