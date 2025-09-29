use std::thread;
use std::time::Duration;
use iceoryx2::port::client::Client;
use iceoryx2::prelude::{ipc, NodeBuilder};
use iceoryx2::prelude::ipc::Service;
use rt_thread::RtThread;
use server_rt_core::core::Core;
use server_rt_core::shared_state::SharedState;
use server_rt_shared::sendable_ptr::SendablePtr;
use server_rt_shared::{MainRequest, MainResponse, RtRequest, RtResponse, StabilizationStatus};

pub(crate) const MAX_LOG_LENGTH: usize = 1024;

fn main() {
    lock_memory();

    //
    // messaging
    //

    let node = NodeBuilder::new().create::<ipc::Service>().unwrap();

    let main_service = node
        .service_builder(&"main_thread".try_into().unwrap())
        .request_response::<RtRequest<MAX_LOG_LENGTH>, MainResponse>()
        .open_or_create().unwrap();

    let main_server = main_service.server_builder().create().unwrap();

    let client_service = node
        .service_builder(&"rt_thread".try_into().unwrap())
        .request_response::<MainRequest, RtResponse>()
        .open_or_create().unwrap();

    let rt_client = client_service.client_builder().create().unwrap();

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

            let main_service = node
                .service_builder(&"main_thread".try_into().unwrap())
                .request_response::<RtRequest<MAX_LOG_LENGTH>, MainResponse>()
                .open_or_create().unwrap();
            let main_client = main_service.client_builder().create().unwrap();

            let client_service = node
                .service_builder(&"rt_thread".try_into().unwrap())
                .request_response::<MainRequest, RtResponse>()
                .open_or_create().unwrap();
            let rt_server = client_service.server_builder().create().unwrap();

            let mut core = Core::new(shared_state, main_client, rt_server);
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
        println!(".");
        thread::sleep(Duration::from_millis(500));
        while let Ok(Some(message)) = main_server.receive() {
            println!("Received message: {:?}", message);
            let response = message.loan_uninit().unwrap();
            let response = match message.payload() {
                RtRequest::StabilityChanged(details) => {
                    if matches!(details, StabilizationStatus::Stable) {
                        stabilized = true;
                    }
                    response.write_payload(MainResponse::Ack)
                }
                _ => {
                    response.write_payload(MainResponse::Nack)
                }
            };
            response.send().unwrap();
        }

        if stabilized || stabilized_ticker > 20 {
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
        send_rt_request_shutdown(&mut message_index, &rt_client);
        handle.join().unwrap();
        return;
    }
    println!("RT system stabilized");

    //
    // io
    //

    init_io();
    send_rt_io_ready(&mut message_index, &rt_client);
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

        if let Ok(pending_response) = rt_client.send_copy(MainRequest::Ping) {
            while let Ok(Some(response)) = pending_response.receive() {
                println!("Received response: {:?}", response);
                match response.payload() {
                    RtResponse::Pong => {
                        println!("PONG");
                        pong_counter += 1;
                    }
                }
            }
        }

        message_index += 1;

        while let Some(active_request) = main_server.receive().unwrap() {
            println!("Received message: {:?}", active_request);
            match active_request.payload() {
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
            }
            let response = active_request.loan_uninit().unwrap();
            let response = response.write_payload(MainResponse::Ack);
            response.send().unwrap();
        }

        if pong_counter > 10 {
            send_rt_request_shutdown(&mut message_index, &rt_client);
            break;
        }

        thread::sleep(Duration::from_millis(1000));
    }

    let result = handle.join();
    println!("RT thread result: {:?}", result);

    println!("RT system stopped");

    drop(boxed_shared_state);
}

fn send_rt_io_ready(
    message_index: &mut usize,
    rt_client: &Client<Service, MainRequest, (), RtResponse, ()>,
) {
    let _response = rt_client.send_copy(MainRequest::EnableIo)
        .expect("Failed to send message");

    *message_index += 1;
}

fn send_rt_request_shutdown(
    message_index: &mut usize,
    rt_client: &Client<Service, MainRequest, (), RtResponse, ()>,
) {
    let _response = rt_client.send_copy(MainRequest::RequestShutdown)
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
