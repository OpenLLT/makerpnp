#![no_std]

extern crate alloc;
extern crate rt_circular_buffer;
extern crate server_rt_shared;

use server_rt_shared::IoStatus;

pub mod core;

// Define communication structures with careful memory layout for RT safety
#[repr(C)]
pub struct SharedState {
    // Using a raw u8 instead of atomic for deterministic latency
    io_status: IoStatus,
    thread_timing_stabilized: bool,
    shutdown_requested: bool,

    latency_stats: [i32; 100],
}

impl SharedState {
    //
    // public api to be used from server
    //

    pub const fn new() -> Self {
        Self {
            io_status: IoStatus::Pending,
            thread_timing_stabilized: false,
            shutdown_requested: false,
            latency_stats: [0; 100],
        }
    }

    pub const fn set_io_status(&mut self, io_status: IoStatus) {
        self.io_status = io_status;
    }

    pub const fn is_stabilized(&self) -> bool {
        self.thread_timing_stabilized
    }

    pub const fn request_shutdown(&mut self) {
        self.shutdown_requested = true;
    }

    /// Safety: no locking here, the values could be updated by the RT thread
    pub fn get_latency_stats(&self) -> &[i32; 100] {
        &self.latency_stats
    }

    //
    // private api, only to be used from this crate
    //

    const fn get_io_status(&self) -> IoStatus {
        self.io_status
    }

    const fn set_stabilized(&mut self, stabilized: bool) {
        self.thread_timing_stabilized = stabilized;
    }

    const fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    const fn set_latency_stats(&mut self, latency_stats: [i32; 100]) {
        self.latency_stats = latency_stats;
    }
}
