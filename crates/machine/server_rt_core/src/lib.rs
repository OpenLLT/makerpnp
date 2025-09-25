#![no_std]

extern crate alloc;
extern crate server_rt_shared;

use server_rt_shared::IoStatus;

#[repr(C)]
pub struct Core {
    shared_state: *mut SharedState,
}

impl Core {
    pub fn new(shared_state_ptr: *mut SharedState) -> Self {
        Self {
            shared_state: shared_state_ptr,
        }
    }

    pub fn start(&mut self) {
        // this will be called once, when the rt thread is started

        // todo call run every 1ms.
    }

    pub fn run(&mut self) {
        // this will be called at a frequency of 1000hz by the `start` method.

        // Safely access shared state without atomics
        let shared_state = unsafe { &mut *self.shared_state };

        // wait for ready signal from main thread
        if !matches!(shared_state.get_io_status(), IoStatus::Ready) {
            return;
        }

        self.process_rt_tasks(shared_state);
    }

    pub fn process_rt_tasks(&mut self, _shared_state: &mut SharedState) {}
}

pub mod rt_ffi {
    use alloc::boxed::Box;

    use crate::{Core, SharedState};

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
