// A wrapper around a raw pointer that is explicitly marked as Send
// This is safe because we ensure proper synchronization between threads
// Only valid when the sender and receiver share the same memory space
// e.g. when using 'mlockall' in the sender before spawning the receiver thread.
pub struct SendablePtr<T>(*mut T);

// Implementing Send is unsafe and requires careful consideration
// We're asserting that it's safe to transfer this pointer between threads
// This is true if we ensure proper synchronization in how we access the data
unsafe impl<T> Send for SendablePtr<T> {}

impl<T> SendablePtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        SendablePtr(ptr)
    }

    pub fn get(&self) -> *mut T {
        self.0
    }
}
