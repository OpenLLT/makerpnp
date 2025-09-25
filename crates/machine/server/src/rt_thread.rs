use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::thread::{self, JoinHandle};

use libc::{self, PTHREAD_EXPLICIT_SCHED, SCHED_FIFO, pthread_attr_t, sched_param};
use server_rt_shared::sendable_ptr::SendablePtr;

// Type for thread entry function
type ThreadEntryFn<T> = extern "C" fn(*mut c_void) -> T;

// A generic RT thread that can work with any RT core
pub struct RtThread<T, R: Send> {
    thread_handle: Option<JoinHandle<R>>,
    priority: i32,
    _phantom: PhantomData<T>, // To mark the type parameter without using it
}

impl<T, R: Send + 'static> RtThread<T, Result<R, RtThreadError>> {
    pub fn new(priority: i32) -> Self {
        Self {
            thread_handle: None,
            priority,
            _phantom: PhantomData,
        }
    }

    pub fn start<F>(
        &mut self,
        data_ptr: *mut T,
        create_fn: F,
        thread_entry: ThreadEntryFn<R>,
    ) -> Result<(), RtThreadError>
    where
        F: FnOnce(*mut T) -> *mut c_void + Send + 'static,
        T: 'static,
    {
        // First ensure memory is locked to prevent paging
        unsafe {
            if libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) != 0 {
                eprintln!(
                    "Failed to lock memory with mlockall: {}",
                    std::io::Error::last_os_error()
                );
                return Err(RtThreadError::FailedToLockMemory);
            }
        }

        // Create a SendablePtr to allow the pointer to cross thread boundaries
        let sendable_ptr = SendablePtr::new(data_ptr);
        let priority = self.priority;

        // Spawn thread that will configure itself for RT
        self.thread_handle = Some(thread::spawn(move || {
            // Get raw pointer back
            let data_ptr = sendable_ptr.get();

            // Call the provided create function to transform data_ptr into the appropriate core
            let thread_data = create_fn(data_ptr);

            // Initialize RT thread attributes
            let mut attr: pthread_attr_t = unsafe { mem::zeroed() };
            let mut param: sched_param = unsafe { mem::zeroed() };

            unsafe {
                // Initialize pthread attributes
                if libc::pthread_attr_init(&mut attr) != 0 {
                    eprintln!("Failed to initialize pthread attributes");
                    return Err(RtThreadError::FailedToInitialisePThreadAttributes);
                }

                // Set scheduling policy to SCHED_FIFO (real-time)
                if libc::pthread_attr_setschedpolicy(&mut attr, SCHED_FIFO) != 0 {
                    eprintln!("Failed to set scheduling policy");
                    return Err(RtThreadError::FailedToSetSchedulingPolicy);
                }

                // Set priority (1-99 for RT, higher = more priority)
                param.sched_priority = priority;
                if libc::pthread_attr_setschedparam(&mut attr, &param) != 0 {
                    eprintln!("Failed to set scheduling parameters");
                    return Err(RtThreadError::FailedToSetSchedulingParameters);
                }

                // Explicitly set scheduling attributes (don't inherit)
                if libc::pthread_attr_setinheritsched(&mut attr, PTHREAD_EXPLICIT_SCHED) != 0 {
                    eprintln!("Failed to set explicit scheduling");
                    return Err(RtThreadError::FailedToSetExcplicitScheduling);
                }

                // Apply RT scheduling to current thread
                let thread_id = libc::pthread_self();
                if libc::pthread_setschedparam(thread_id, SCHED_FIFO, &param) != 0 {
                    eprintln!(
                        "Failed to set thread scheduling parameters: {}",
                        std::io::Error::last_os_error()
                    );
                    return Err(RtThreadError::FailedToApplySchedulingParameters);
                }
            }

            println!("RT thread starting with priority {}", priority);

            // Call the provided thread entry function with the core pointer
            let result = thread_entry(thread_data);

            Ok(result)
        }));

        Ok(())
    }

    pub fn join(self) -> std::thread::Result<Result<R, RtThreadError>> {
        let thread_handle = self.thread_handle.unwrap();
        let result = thread_handle.join();

        result
    }
}

#[derive(Debug)]
pub enum RtThreadError {
    FailedToLockMemory,
    FailedToInitialisePThreadAttributes,
    FailedToSetSchedulingPolicy,
    FailedToSetSchedulingParameters,
    FailedToSetExcplicitScheduling,
    FailedToApplySchedulingParameters,
}
