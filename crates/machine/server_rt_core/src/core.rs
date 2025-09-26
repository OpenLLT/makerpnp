use rt_circular_buffer::CircularBuffer;
use rt_spsc::{Receiver, Sender};
use rt_time::{get_time_ns, sleep_until_ns};
use server_rt_shared::{IoStatus, MainRequest, MainResponse, Message, Response, RtRequest, RtResponse};

use crate::SharedState;

const LATENCY_BUFFER_SIZE: usize = 100;
/// 50 microseconds = 50_000 nanoseconds
const ACCEPTABLE_DEVIATION_NS: u32 = 200_000;
const LATENCY_DEVIATION_THRESHOLD_PERCENTAGE: u8 = 95;

#[repr(C)]
pub struct Core<'a, const Q1: usize, const Q2: usize, const MAX_LOG_LENGTH: usize> {
    shared_state: &'static mut SharedState,
    done: bool,
    tick: usize,

    /// Circular buffer to store latency measurements
    /// Note: maximum recorded latency is limited by using a u32 for speed here
    latency_buffer: CircularBuffer<i32, LATENCY_BUFFER_SIZE>,
    pub sender: Sender<'a, Message<RtRequest<{ MAX_LOG_LENGTH }>, RtResponse>, { Q1 }>,
    pub receiver: Receiver<'a, Message<MainRequest, MainResponse>, { Q2 }>,
}

impl<'a, const Q1: usize, const Q2: usize, const MAX_LOG_LENGTH: usize> Core<'a, Q1, Q2, MAX_LOG_LENGTH> {
    pub fn new(
        shared_state: &'static mut SharedState,
        sender: Sender<'a, Message<RtRequest<MAX_LOG_LENGTH>, RtResponse>, Q1>,
        receiver: Receiver<'a, Message<MainRequest, MainResponse>, Q2>,
    ) -> Self {
        Self {
            shared_state,
            done: false,
            tick: 0,
            latency_buffer: CircularBuffer::new(),
            sender,
            receiver,
        }
    }

    pub fn start(&mut self) -> usize {
        // this will be called once, when the rt thread is started

        // Calculate initial wake time
        let mut next_wake_ns = get_time_ns();
        let period_ns: u64 = 1_000_000; // 1ms in nanoseconds

        // Main real-time loop
        loop {
            self.tick += 1;
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

        // TODO we shouldn't try and receive messages since we can't respond if the sender cannot send.

        if let Some(message) = self.receiver.try_receive() {
            match message {
                Message::Request(request) => match request.payload {
                    MainRequest::Ping => {
                        let response = Message::Response(Response {
                            request_reference: request.index,
                            payload: RtResponse::Pong,
                        });
                        self.sender
                            .try_send(response)
                            .expect("sent");
                    }
                },
                Message::Response(response) => match response.payload {
                    MainResponse::None => {}
                },
            }
        }

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
        if self.tick >= 5000 {
            self.done = true;
        }
    }
}
