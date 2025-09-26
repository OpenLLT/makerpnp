use core::mem::MaybeUninit;

/// A fixed-size circular buffer that operates in constant time
/// and doesn't require dynamic memory allocation.
#[derive(Debug)]
pub struct CircularBuffer<T, const N: usize> {
    /// Internal buffer to store elements
    buffer: [MaybeUninit<T>; N],
    /// Current write position
    position: usize,
    /// Number of elements currently in the buffer
    count: usize,
}

impl<T: Copy + Default, const N: usize> CircularBuffer<T, N> {
    /// Creates a new, empty circular buffer
    pub const fn new() -> Self {
        Self {
            // Initialize with default values
            buffer: unsafe {
                // safe as we prevent access the uninitialised elements
                MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init()
            },
            position: 0,
            count: 0,
        }
    }

    /// Pushes a value into the buffer, overwriting the oldest value if full
    pub fn push(&mut self, value: T) {
        self.buffer[self.position].write(value);

        self.position += 1;
        if self.position >= N {
            self.position = 0;
        }

        // Update count (up to capacity)
        if self.count < N {
            self.count += 1;
        }
    }

    /// Returns the number of elements currently in the buffer
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Returns true if the buffer is empty
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Gets the element at the specified index
    ///
    /// # Safety
    ///
    /// This function doesn't perform bounds checking. The caller must
    /// ensure that `index < self.count`.
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        debug_assert!(index < self.count, "Index out of bounds");
        unsafe { self.buffer[index].assume_init_ref() }
    }

    /// Returns the sum of all elements in the buffer.
    pub fn sum(&self) -> T
    where
        T: core::ops::Add<Output = T> + Copy,
    {
        // We need a starting value for the sum
        if self.is_empty() {
            // Create a zero value using transmute
            // This is safe because we're only using it for numeric types
            // where a zeroed value is valid
            unsafe {
                let zero: T = core::mem::zeroed();
                return zero;
            }
        }

        // Start with the first element
        let mut sum = *unsafe { self.get_unchecked(0) };

        // Add the remaining elements
        for i in 1..self.count {
            sum = sum + *unsafe { self.get_unchecked(i) };
        }

        sum
    }

    /// Returns an iterator over the elements in the buffer
    pub fn iter(&self) -> CircularBufferIter<'_, T, N> {
        CircularBufferIter {
            buffer: self,
            index: 0,
        }
    }
}

// Implement Drop to properly drop all initialized elements
impl<T, const N: usize> Drop for CircularBuffer<T, N> {
    fn drop(&mut self) {
        for i in 0..self.count {
            // Safety: We're only dropping elements that have been initialized
            unsafe {
                self.buffer[i].assume_init_drop();
            }
        }
    }
}

/// Iterator for CircularBuffer
pub struct CircularBufferIter<'a, T, const N: usize> {
    buffer: &'a CircularBuffer<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for CircularBufferIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.buffer.count {
            let item = unsafe { self.buffer.buffer[self.index].assume_init_ref() };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
