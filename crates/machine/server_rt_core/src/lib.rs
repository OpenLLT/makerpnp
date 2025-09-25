#![no_std]

extern crate alloc;
extern crate libc;
extern crate server_rt_shared;

use server_rt_shared::IoStatus;

pub mod core;
pub mod rt_time;


// Define communication structures with careful memory layout for RT safety
#[repr(C)]
pub struct SharedState {
    // Using a raw u8 instead of atomic for deterministic latency
    pub io_status: IoStatus,
}

impl SharedState {
    //
    // public api to be used from server
    //

    pub const fn new() -> Self {
        Self {
            io_status: IoStatus::Pending,
        }
    }

    pub const fn set_io_status(&mut self, io_status: IoStatus) {
        self.io_status = io_status;
    }

    //
    // private api, only to be used from this crate
    //

    const fn get_io_status(&self) -> IoStatus {
        self.io_status
    }
}
