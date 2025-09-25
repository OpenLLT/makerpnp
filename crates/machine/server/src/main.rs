use std::ffi::c_void;
use std::thread;
use std::time::Duration;

use server_rt_core::SharedState;
use server_rt_core::core::core_ffi::{create_core_for_thread, rt_thread_entry};
use server_rt_shared::IoStatus;

use crate::rt_thread::{RtThread, RtThreadError};

mod rt_thread;

fn main() {
    let shared_state = Box::new(SharedState::new());
    let shared_state_ptr = Box::into_raw(shared_state);

    // Create RT thread with priority 80 (out of 1-99)
    let mut rt_thread = RtThread::<SharedState, Result<usize, RtThreadError>>::new(80);
    rt_thread
        .start(
            shared_state_ptr,
            |ptr| create_core_for_thread(ptr as *mut c_void),
            rt_thread_entry,
        )
        .unwrap_or_else(|error| {
            println!("RT thead failed to start, cause: {:?}", error);
        });
    println!("RT thread launched");

    // TODO busy wait until rt thread sets some state

    init_io();

    // Signal RT thread that IO is ready
    set_rt_ready(shared_state_ptr);

    println!("RT system started and running");

    for index in 10..0 {
        // Non-RT processing
        println!("Processing...");
        thread::sleep(Duration::from_millis(1000));
        println!("{}", index);
    }

    let result = rt_thread.join();
    println!("RT thread result: {:?}", result);

    println!("RT system stopped");
}

fn init_io() {
    // Initialize non-RT IO subsystems
    println!("Initializing IO...");
    thread::sleep(Duration::from_millis(100)); // Simulating IO initialization
    println!("IO initialized");
}

fn set_rt_ready(shared_state_ptr: *mut SharedState) {
    // Signal to RT thread that IO is ready
    unsafe {
        (*shared_state_ptr).set_io_status(IoStatus::Ready);
    }
    println!("IO Ready signal sent to RT thread");
}
