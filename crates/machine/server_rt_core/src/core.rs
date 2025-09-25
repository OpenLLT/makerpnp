use alloc::boxed::Box;
use core::ffi::c_void;

use server_rt_shared::IoStatus;

use crate::SharedState;

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

// Create a new Core instance and return opaque pointer for rt_thread_entry
#[unsafe(no_mangle)]
pub extern "C" fn create_core_for_thread(shared_state_ptr: *mut c_void) -> *mut c_void {
    let core = Box::new(Core::new(shared_state_ptr as *mut crate::SharedState));
    Box::into_raw(core) as *mut c_void
}
