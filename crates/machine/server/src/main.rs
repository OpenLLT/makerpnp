use std::thread;
use std::time::Duration;

use server_rt_core::SharedState;
use server_rt_core::rt_ffi::{core_new, core_run, core_start};
use server_rt_shared::IoStatus;
use server_rt_shared::sendable_ptr::SendablePtr;

fn main() {
    let shared_state = Box::new(SharedState::new());
    let shared_state_ptr = Box::into_raw(shared_state);

    start_rt_thread(shared_state_ptr);

    // TODO busy wait until rt thread sets some state

    init_io();

    // Signal RT thread that IO is ready
    set_rt_ready(shared_state_ptr);

    println!("RT system started and running");

    loop {
        // Non-RT processing
        thread::sleep(Duration::from_millis(1000));
        print!(".");
    }
}

fn start_rt_thread(shared_state_ptr: *mut SharedState) {
    let sendable_ptr = SendablePtr::new(shared_state_ptr);
    // Spawn RT thread with high priority
    thread::spawn(move || {
        let shared_state_ptr = sendable_ptr.get();

        // Create RT core instance
        let core_ptr = core_new(shared_state_ptr);

        // Initialize RT core
        unsafe { core_start(core_ptr) };

        // Set thread scheduling parameters for real-time
        // This would use pthread_setschedparam on Linux to set
        // SCHED_FIFO or SCHED_RR with high priority

        println!("RT thread starting");

        // Simple timer loop - in production would use POSIX timers
        // or kernel RT timers for precise timing
        loop {
            unsafe { core_run(core_ptr) };

            // Sleep for ~1ms (1000Hz frequency)
            // In a real implementation, you'd use high-precision sleep
            // mechanisms like clock_nanosleep with TIMER_ABSTIME
            thread::sleep(Duration::from_micros(999));
        }
    });
}

fn init_io() {
    // TODO io initialisation
}

fn set_rt_ready(shared_state_ptr: *mut SharedState) {
    // Signal to RT thread that IO is ready
    unsafe {
        (*shared_state_ptr).set_io_status(IoStatus::Ready);
    }
    println!("IO Ready signal sent to RT thread");
}
