use std::thread;
use std::time::Duration;

use server_rt_core::SharedState;
use server_rt_shared::IoStatus;

use crate::rt_thread::RtThread;

mod rt_thread;

fn main() {
    let shared_state = Box::new(SharedState::new());
    let shared_state_ptr = Box::into_raw(shared_state);

    // Create RT thread with priority 80 (out of 1-99)
    let mut rt_thread = RtThread::new(80);
    start_rt_thread(&mut rt_thread, shared_state_ptr);

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

fn start_rt_thread(rt_thread: &mut RtThread, shared_state_ptr: *mut SharedState) {
    // Start the RT thread with shared state
    rt_thread.start(shared_state_ptr);
    println!("RT thread launched");
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
