use std::thread;
use std::time::Duration;

use rt_spsc::{Sender, make_static_channel};
use rt_thread::RtThread;
use server_rt_core::core::Core;
use server_rt_core::shared_state::SharedState;
use server_rt_shared::sendable_ptr::SendablePtr;
use server_rt_shared::{MainRequest, MainResponse, Message, Request, RtRequest, RtResponse, StabilizationStatus};

pub(crate) const QUEUE_SIZE: usize = 1024;
pub(crate) const MAX_LOG_LENGTH: usize = 1024;

fn main() {
    lock_memory();

    //
    // message channels
    //

    let (main_channel, boxed_main_channel) = make_static_channel::<QUEUE_SIZE, Message<MainRequest, MainResponse>>();
    let (main_to_rt_sender, main_to_rt_receiver) = main_channel.split();

    let (rt_channel, boxed_rt_channel) =
        make_static_channel::<QUEUE_SIZE, Message<RtRequest<MAX_LOG_LENGTH>, RtResponse>>();
    let (rt_to_main_sender, rt_to_main_receiver) = rt_channel.split();

    let mut message_index = 0;

    //
    // shared state
    //

    let shared_state = Box::new(SharedState::new());
    let shared_state_ptr = Box::into_raw(shared_state);
    let boxed_shared_state = unsafe { Box::from_raw(shared_state_ptr) };

    //
    // make rt thread
    //

    // Create RT thread with priority 80 (out of 1-99)
    let handle = RtThread::spawn(80, {
        let shared_state_ptr = SendablePtr::new(shared_state_ptr);
        move || {
            // Safely access shared state without atomics
            let shared_state = unsafe { &mut *shared_state_ptr.get() };

            let mut core = Core::new(shared_state, rt_to_main_sender, main_to_rt_receiver);
            core.start()
        }
    });
    println!("RT thread launched");

    // create a shared reference to the shared state from the pointer
    let shared_state = unsafe { &mut *shared_state_ptr };

    println!("Waiting for RT system to stabilize...");
    let mut stabilized_ticker = 0;
    let mut stabilized = false;
    loop {
        thread::sleep(Duration::from_millis(500));
        while let Some(message) = rt_to_main_receiver.try_receive() {
            println!("Received message: {:?}", message);
            match message {
                Message::Request(request) => match request.payload {
                    RtRequest::StabilityChanged(details) => {
                        if matches!(details, StabilizationStatus::Stable) {
                            stabilized = true;
                            break;
                        }
                    }
                    _ => {
                        // other requests ignored during stabilization
                    }
                },
                _ => {
                    // responses ignored during stabilization
                }
            }
        }

        if stabilized_ticker > 20 {
            break;
        }
        stabilized_ticker += 1;
    }

    let latency_stats = shared_state.get_latency_stats().clone();
    println!("RT recent latency values (ns): {:?}", latency_stats);

    let average_latency = latency_stats.iter().sum::<i32>() / latency_stats.len() as i32;
    println!("RT average latency: {}ns", average_latency);

    if !stabilized {
        eprintln!("RT system failed to stabilize");
        send_rt_request_shutdown(&main_to_rt_sender, &mut message_index);
        handle.join().unwrap();
        return;
    }
    println!("RT system stabilized");

    //
    // io
    //

    init_io();
    send_rt_io_ready(&main_to_rt_sender, &mut message_index);
    println!("IO Ready signal sent to RT thread");

    println!("RT system started and running");

    let mut pong_counter = 0;

    loop {
        if handle.is_finished() {
            break;
        }

        //
        // send ping
        //

        let message = Message::Request(Request {
            index: message_index,
            payload: MainRequest::Ping,
        });
        main_to_rt_sender
            .try_send(message)
            .expect("Failed to send message");
        message_index += 1;

        while let Some(message) = rt_to_main_receiver.try_receive() {
            println!("Received message: {:?}", message);
            match message {
                Message::Request(request) => match request.payload {
                    RtRequest::Log(log_buffer) => {
                        println!("LOG: {:?}", log_buffer);
                    }
                    RtRequest::Shutdown => break,
                    RtRequest::StabilityChanged(status) => match status {
                        StabilizationStatus::Stable => {
                            println!("RT system stabilized");
                        }
                        StabilizationStatus::Unstable => {
                            println!("RT system unstable");
                        }
                    },
                },
                Message::Response(response) => match response.payload {
                    RtResponse::Pong => {
                        println!("PONG");
                        pong_counter += 1;
                    }
                },
            }
        }

        if pong_counter > 10 {
            send_rt_request_shutdown(&main_to_rt_sender, &mut message_index);
        }

        thread::sleep(Duration::from_millis(1000));
    }

    let result = handle.join();
    println!("RT thread result: {:?}", result);

    println!("RT system stopped");

    drop(boxed_rt_channel);
    drop(boxed_main_channel);
    drop(boxed_shared_state);
}

fn send_rt_io_ready(main_to_rt_sender: &Sender<Message<MainRequest, MainResponse>, 1024>, message_index: &mut usize) {
    // Signal RT thread that IO is ready
    let message = Message::Request(Request {
        index: *message_index,
        payload: MainRequest::EnableIo,
    });
    main_to_rt_sender
        .try_send(message)
        .expect("Failed to send message");
    *message_index += 1;
}

fn send_rt_request_shutdown(
    main_to_rt_sender: &Sender<Message<MainRequest, MainResponse>, 1024>,
    message_index: &mut usize,
) {
    let message = Message::Request(Request {
        index: *message_index,
        payload: MainRequest::RequestShutdown,
    });
    main_to_rt_sender
        .try_send(message)
        .expect("Failed to send message");

    *message_index += 1;
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
