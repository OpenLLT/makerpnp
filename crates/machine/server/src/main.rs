use std::ffi::c_void;
use std::thread;
use std::time::Duration;

use server_rt_core::SharedState;
use server_rt_core::core::core_ffi::{create_core_for_thread, rt_thread_entry};
use server_rt_shared::IoStatus;

use crate::rt_thread::RtThread;

mod rt_thread;

fn main() {
    lock_memory();

    let shared_state = Box::new(SharedState::new());
    let shared_state_ptr = Box::into_raw(shared_state);

    // Create RT thread with priority 80 (out of 1-99)
    let handle = RtThread::spawn(
        80,
        shared_state_ptr,
        |ptr| create_core_for_thread(ptr as *mut c_void),
        rt_thread_entry,
    );
    println!("RT thread launched");

    // create a shared reference to the shared state from the pointer
    let shared_state = unsafe { &mut *shared_state_ptr };

    println!("Waiting for RT system to stabilize...");
    let mut stabilized_ticker = 0;
    loop {
        thread::sleep(Duration::from_millis(500));
        if shared_state.is_stabilized() || stabilized_ticker > 20 {
            break;
        }
        stabilized_ticker += 1;
    }

    let latency_stats = shared_state.get_latency_stats().clone();
    println!("RT recent latency values (ns): {:?}", latency_stats);

    let average_latency = latency_stats.iter().sum::<i32>() / latency_stats.len() as i32;
    println!("RT average latency: {}ns", average_latency);

    if !shared_state.is_stabilized() {
        eprintln!("RT system failed to stabilize");
        shared_state.request_shutdown();
        handle.join().unwrap();
        return;
    }
    println!("RT system stabilized");

    init_io();

    // Signal RT thread that IO is ready
    shared_state.set_io_status(IoStatus::Ready);
    println!("IO Ready signal sent to RT thread");

    println!("RT system started and running");

    for index in (0..=10).rev() {
        // Non-RT processing
        println!("Processing...");
        thread::sleep(Duration::from_millis(1000));
        println!("{}", index);

        if handle.is_finished() {
            break;
        }
    }

    let result = handle.join();
    println!("RT thread result: {:?}", result);

    println!("RT system stopped");
}

fn init_io() {
    // Initialize non-RT IO subsystems
    println!("Initializing IO...");
    thread::sleep(Duration::from_millis(100)); // Simulating IO initialization
    println!("IO initialized");
}

/// ensure memory is locked to prevent paging
fn lock_memory() {
    unsafe {
        if libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) != 0 {
            panic!(
                "Failed to lock memory with mlockall: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}
