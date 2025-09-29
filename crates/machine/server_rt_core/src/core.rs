use core::time::Duration;
use iceoryx2::node::Node;
use iceoryx2::port::client::Client;
use iceoryx2::port::server::Server;
use iceoryx2::prelude::ipc::Service;
use rt_circular_buffer::CircularBuffer;
use rt_time::get_time_ns;
use server_rt_shared::{
    IoStatus, MainRequest, MainResponse, RtRequest, RtResponse, StabilizationStatus,
};

use crate::shared_state::SharedState;

const LATENCY_BUFFER_SIZE: usize = 100;
/// 50 microseconds = 50_000 nanoseconds
const ACCEPTABLE_DEVIATION_NS: u32 = 200_000;
const LATENCY_DEVIATION_THRESHOLD_PERCENTAGE: u8 = 95;

#[repr(C)]
pub struct Core<const MAX_LOG_LENGTH: usize> {
    shared_state: &'static mut SharedState,

    done: bool,
    tick: usize,
    io_status: IoStatus,

    /// Circular buffer to store latency measurements
    /// Note: maximum recorded latency is limited by using a u32 for speed here
    latency_buffer: CircularBuffer<i32, LATENCY_BUFFER_SIZE>,

    stabilization: StabilizationStatus,
    message_index: usize,
    main_client: Client<Service, RtRequest<{ MAX_LOG_LENGTH }>, (), MainResponse, ()>,
    rt_server: Server<Service, MainRequest, (), RtResponse, ()>,
    rt_node: Node<Service>,
}

impl<const MAX_LOG_LENGTH: usize> Core<MAX_LOG_LENGTH> {
    pub fn new(
        shared_state: &'static mut SharedState,
        main_client: Client<Service, RtRequest<{ MAX_LOG_LENGTH }>, (), MainResponse, ()>,
        rt_server: Server<Service, MainRequest, (), RtResponse, ()>,
        rt_node: Node<Service>,
    ) -> Self {
        Self {
            shared_state,
            done: false,
            tick: 0,
            io_status: IoStatus::Pending,
            latency_buffer: CircularBuffer::new(),
            main_client,
            rt_server,
            rt_node,
            stabilization: StabilizationStatus::Unstable,
            message_index: 0,
        }
    }

    pub fn start(&mut self) -> usize {
        // this will be called once, when the rt thread is started

        // Calculate initial wake time
        let period_ns: u64 = 1_000_000; // 1ms in nanoseconds

        // Main real-time loop
        loop {
            self.tick += 1;
            // Run core processing
            self.run();

            if self.done {
                break;
            }

            let sleep_ns = get_time_ns();
            if self.rt_node.wait(Duration::from_nanos(period_ns)).is_err() {
                break;
            }
            let wake_ns = get_time_ns();

            self.determine_stability(
                sleep_ns,
                wake_ns,
                LATENCY_DEVIATION_THRESHOLD_PERCENTAGE,
                ACCEPTABLE_DEVIATION_NS,
                period_ns,
            );
            self.update_latency_stats();
        }

        self.tick
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
        sleep_ns: u64,
        wake_ns: u64,
        threshold_percentage: u8,
        acceptable_deviation: u32,
        period_ns: u64,
    ) {
        // Calculate latency deviation (signed value)
        // This represents how far off we are from the target wakeup time
        let latency_deviation = wake_ns as i64 - sleep_ns as i64 - period_ns as i64;

        // Convert to i32 for storage in the buffer (safe for reasonable latencies)
        let latency_deviation_i32 = latency_deviation as i32;

        // Record the latency deviation in the circular buffer
        self.latency_buffer
            .push(latency_deviation_i32);

        // TODO determine which method is actually best
        //      it is observed that the 'percentage' method results in more stability change notification messages

        let state = if false {
            // Calculate statistics for acceptability check
            let buffer_len = self.latency_buffer.len();

            // Check if each individual latency value is within acceptable range
            let acceptable_entries = self
                .latency_buffer
                .iter()
                .filter(|&&x| x.abs() <= acceptable_deviation as i32)
                .count();

            let stability_threshold = (buffer_len * threshold_percentage as usize) / 100;
            if acceptable_entries >= stability_threshold {
                StabilizationStatus::Stable
            } else {
                StabilizationStatus::Unstable
            }
        } else {
            // Calculate the average absolute latency deviation
            let buffer_len = self.latency_buffer.len();

            // We need to calculate mean absolute deviation
            // First, we need to iterate through values to calculate absolute deviation
            let abs_sum = self
                .latency_buffer
                .iter()
                .fold(0_u32, |acc, &x| acc + x.abs() as u32);

            // Time-constant division (avoid division by zero)
            let avg_abs_deviation = if buffer_len > 0 { abs_sum / buffer_len as u32 } else { 0 };

            // Determine if the average absolute deviation is acceptable
            if avg_abs_deviation <= acceptable_deviation {
                StabilizationStatus::Stable
            } else {
                StabilizationStatus::Unstable
            }
        };

        if state != self.stabilization {
            self.stabilization = state;
            self.send_request(RtRequest::StabilityChanged(state));
        }
    }

    // TODO don't panic if unable to send messages and/or add an unchecked_send_message

    /// Safety: panics if unable to send
    fn send_request(&mut self, payload: RtRequest<{ MAX_LOG_LENGTH }>) {
        self.main_client.send_copy(payload)
            .expect("sent");

        self.message_index += 1;
    }

    pub fn run(&mut self) {
        // this will be called at a frequency of 1000hz by the `start` method.

        while let Ok(Some(request)) = self.rt_server.receive() {
            match request.payload() {
                MainRequest::Ping => {
                    let response = request.loan_uninit().unwrap();
                    let response = response.write_payload(RtResponse::Pong);
                    response.send()
                        .expect("sent");
                }
                MainRequest::RequestShutdown => {
                    self.done = true;
                }
                MainRequest::EnableIo => self.io_status = IoStatus::Ready,
            }
        }

        // wait for ready signal from main thread
        if !matches!(self.io_status, IoStatus::Ready) {
            return;
        }

        self.process_io_tasks();
    }

    pub fn process_io_tasks(&mut self) {}
}
