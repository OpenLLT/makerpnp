// RT timing utilities that don't require std
use core::mem;
use core::ptr;

use libc::{self, CLOCK_MONOTONIC, EINTR, TIMER_ABSTIME, timespec};

// Get current time in nanoseconds using CLOCK_MONOTONIC
pub fn get_time_ns() -> u64 {
    let mut ts: timespec = unsafe { mem::zeroed() };
    unsafe {
        libc::clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
}

// Sleep until specific time using high-precision clock_nanosleep
pub fn sleep_until_ns(target_ns: u64) {
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
