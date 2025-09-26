#![cfg_attr(not(test), no_std)]
extern crate alloc;

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{Ordering, compiler_fence};

pub struct Spsc<T: Send + 'static, const N: usize> {
    _phantom: PhantomData<T>,
    buffer: [MaybeUninit<T>; N],
    read_index: UnsafeCell<usize>,
    write_index: UnsafeCell<usize>,
    is_empty: UnsafeCell<bool>, // Flag to track empty state
    is_split: bool,
}

impl<T: Send + Copy + 'static, const N: usize> Spsc<T, N> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            buffer: [MaybeUninit::uninit(); N],
            read_index: UnsafeCell::new(0),
            write_index: UnsafeCell::new(0),
            is_empty: UnsafeCell::new(true), // Initially empty
            is_split: false,
        }
    }

    pub fn split(&mut self) -> (Sender<T, N>, Receiver<T, N>) {
        if self.is_split {
            panic!("Spsc can only be split once");
        }
        self.is_split = true;

        let self_ptr = self as *mut Self;
        (Sender::new(self_ptr), Receiver::new(self_ptr))
    }
}

pub struct Sender<T: Send + 'static, const N: usize> {
    spcs: *mut Spsc<T, N>,
}

unsafe impl<T: Send + 'static, const N: usize> Send for Sender<T, N> {}

impl<T: Send + 'static, const N: usize> Sender<T, N> {
    fn new(spcs: *mut Spsc<T, { N }>) -> Sender<T, N> {
        Self {
            spcs,
        }
    }

    pub fn try_send(&self, value: T) -> Result<(), T> {
        unsafe {
            let spcs = &mut *self.spcs;

            let write_idx = *spcs.write_index.get();
            let read_idx = *spcs.read_index.get();
            let is_empty = *spcs.is_empty.get();

            let next_write = (write_idx + 1) % N;

            // Check if buffer is full
            if next_write == read_idx && !is_empty {
                return Err(value); // Buffer full
            }

            spcs.buffer[write_idx].write(value);

            // Ensure write completes before updating index
            compiler_fence(Ordering::Release);

            *spcs.write_index.get() = next_write;
            *spcs.is_empty.get() = false; // Not empty after sending

            Ok(())
        }
    }
}

pub struct Receiver<T: Send + 'static, const N: usize> {
    spcs: *mut Spsc<T, N>,
}

unsafe impl<T: Send + 'static, const N: usize> Send for Receiver<T, N> {}

impl<T: Send + 'static, const N: usize> Receiver<T, N> {
    fn new(spcs: *mut Spsc<T, { N }>) -> Receiver<T, N> {
        Self {
            spcs,
        }
    }

    pub fn try_receive(&self) -> Option<T> {
        unsafe {
            let spcs = &mut *self.spcs;

            let write_idx = *spcs.write_index.get();
            let read_idx = *spcs.read_index.get();
            let is_empty = *spcs.is_empty.get();

            if read_idx == write_idx && is_empty {
                return None; // Buffer empty
            }

            // Ensure we read the index before reading the data
            compiler_fence(Ordering::Acquire);

            // Read value
            let value = spcs.buffer[read_idx].assume_init_read();

            let next_read = (read_idx + 1) % N;
            *spcs.read_index.get() = next_read;

            // Check if the buffer is now empty
            *spcs.is_empty.get() = next_read == write_idx;

            Some(value)
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::*;

    #[test]
    fn spsc_test() {
        // given
        let mut spsc = Spsc::<_, 1>::new(); // Now works with capacity of 1
        let (sender, receiver) = spsc.split();

        // when
        sender.try_send(1).expect("not full");
        let result = receiver.try_receive();

        // then
        assert_eq!(result, Some(1));
    }

    #[test]
    fn spsc_receive_on_empty_channel() {
        // given
        let mut spsc = Spsc::<(), 1024>::new(); // Now works with capacity of 1
        let (sender, receiver) = spsc.split();

        // when
        let result = receiver.try_receive();

        // then
        assert_eq!(result, None);
    }

    #[test]
    fn spsc_full_then_empty() {
        // given
        let mut spsc = Spsc::<_, 1>::new();
        let (sender, receiver) = spsc.split();

        // expect
        sender.try_send(1).expect("not full");
        // Should be full now
        assert!(sender.try_send(2).is_err(), "Queue should be full");
        // Read the item
        assert_eq!(receiver.try_receive(), Some(1));
        // Should be empty now
        assert_eq!(receiver.try_receive(), None);
        // Should be able to write again
        sender.try_send(3).expect("not full");
        assert_eq!(receiver.try_receive(), Some(3));
    }

    #[test]
    fn spsc_across_threads() {
        // Create a barrier to synchronize thread startup
        let barrier = Arc::new(Barrier::new(3)); // Main + 2 threads

        // Create the channel
        let mut spsc = Spsc::<i32, 1>::new();
        let (sender, receiver) = spsc.split();

        // Clone barrier for the sender thread
        let sender_barrier = Arc::clone(&barrier);
        // Spawn sender thread
        let sender_thread = thread::spawn(move || {
            // Wait for all threads to be ready
            sender_barrier.wait();

            // Send value
            println!("Sender thread: sending value");
            sender
                .try_send(42)
                .expect("Failed to send value");
            println!("Sender thread: value sent");

            // Return the sender to verify it lived long enough
            sender
        });

        // Clone barrier for the receiver thread
        let receiver_barrier = Arc::clone(&barrier);
        // Spawn receiver thread
        let receiver_thread = thread::spawn(move || {
            // Wait for all threads to be ready
            receiver_barrier.wait();

            // A small delay to ensure the sender has time to send
            // This isn't strictly necessary but helps demonstrate the concept
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Receive value
            println!("Receiver thread: waiting for value");
            let value = receiver.try_receive();
            println!("Receiver thread: received value {:?}", value);

            // Assert the correct value was received
            assert_eq!(value, Some(42));

            // Return the receiver to verify it lived long enough
            receiver
        });

        // Wait for all threads to be ready
        barrier.wait();
        println!("Main thread: all threads ready, test running");

        // Wait for both threads to complete
        let _sender = sender_thread
            .join()
            .expect("Sender thread panicked");
        let _receiver = receiver_thread
            .join()
            .expect("Receiver thread panicked");

        println!("Main thread: test completed successfully");
    }

    #[test]
    fn spsc_blocking_receive() {
        // Create a channel
        let mut spsc = Spsc::<i32, 1>::new();
        let (sender, receiver) = spsc.split();

        // Spawn sender thread that waits briefly before sending
        let sender_thread = thread::spawn(move || {
            // Wait a bit to ensure the receiver is blocking
            std::thread::sleep(std::time::Duration::from_millis(100));

            println!("Sender thread: sending value");
            sender
                .try_send(69)
                .expect("Failed to send value");
            println!("Sender thread: value sent");

            sender
        });

        // Spawn receiver thread that uses a blocking receive
        let receiver_thread = thread::spawn(move || {
            println!("Receiver thread: waiting for value");

            // Implement a simple blocking receive with polling
            let mut value = None;
            for _ in 0..100 {
                // Try up to 100 times
                value = receiver.try_receive();
                if value.is_some() {
                    break;
                }
                // Sleep briefly between attempts
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            println!("Receiver thread: received value {:?}", value);
            assert_eq!(value, Some(69));

            receiver
        });

        // Wait for both threads to complete
        let _sender = sender_thread
            .join()
            .expect("Sender thread panicked");
        let _receiver = receiver_thread
            .join()
            .expect("Receiver thread panicked");

        println!("Main thread: test completed successfully");
    }
}
