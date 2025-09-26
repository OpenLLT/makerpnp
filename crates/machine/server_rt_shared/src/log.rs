use core::mem::MaybeUninit;

#[derive(Debug)]
#[derive(Copy, Clone)]
pub struct LogBuffer<const MAX_LOG_LENGTH: usize> {
    bytes: MaybeUninit<[u8; MAX_LOG_LENGTH]>,
    length: usize,
}

impl<const N: usize> LogBuffer<N> {
    pub fn from_raw_str(s: &str) -> Self {
        let length = s.bytes().len().max(N);
        let mut buffer = Self {
            bytes: MaybeUninit::uninit(),
            length,
        };

        unsafe {
            // Get a pointer to the uninitialized array
            let dst = buffer.bytes.as_mut_ptr() as *mut u8;

            // Copy bytes from the string (up to length)
            let src = s.as_bytes().as_ptr();
            core::ptr::copy_nonoverlapping(src, dst, length);
        }

        buffer
    }
}
