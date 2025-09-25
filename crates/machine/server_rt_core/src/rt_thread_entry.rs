use alloc::boxed::Box;

use libc::c_void;

use crate::core::Core;
use crate::rt_time::{get_time_ns, sleep_until_ns};

// Generic RT thread entry point
pub type RtThreadEntryFn = extern "C" fn(*mut c_void) -> ();

// Entry point for standard Core RT thread
#[unsafe(no_mangle)]
pub extern "C" fn rt_thread_entry(data_ptr: *mut c_void) {
    // This function receives an opaque pointer and manages the real-time loop
    // The pointer should be a properly initialized Core instance
    let core_ptr = data_ptr;

    // Initialize RT core
    unsafe { crate::rt_ffi::core_start(core_ptr as *mut Core) };

    // Calculate initial wake time
    let mut next_wake_ns = get_time_ns();
    let period_ns: u64 = 1_000_000; // 1ms in nanoseconds

    // Main real-time loop
    loop {
        // Run core processing
        unsafe { crate::rt_ffi::core_run(core_ptr as *mut Core) };

        // Calculate next wake time
        next_wake_ns += period_ns;

        // Sleep until next wake time using clock_nanosleep with TIMER_ABSTIME
        // for deterministic timing
        sleep_until_ns(next_wake_ns);
    }
}
