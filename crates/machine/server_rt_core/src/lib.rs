#![no_std]

extern crate alloc;
extern crate libc;
extern crate server_rt_shared;

use server_rt_shared::IoStatus;

pub mod core;
pub mod rt_thread_entry;
pub mod rt_time;

pub mod rt_ffi {
    use alloc::boxed::Box;

    use crate::SharedState;
    use crate::core::Core;

    // Export a C-compatible interface for the RT thread to call
    #[unsafe(no_mangle)]
    pub extern "C" fn core_new(shared_state_ptr: *mut SharedState) -> *mut Core {
        let core = Box::new(Core::new(shared_state_ptr));
        Box::into_raw(core)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn core_run(core_ptr: *mut Core) {
        let core = unsafe { &mut *core_ptr };
        core.run();
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn core_start(core_ptr: *mut Core) {
        let core = unsafe { &mut *core_ptr };
        core.start();
    }
}

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
