use core::fmt::{Display, Formatter};
use core::mem::MaybeUninit;
use iceoryx2::prelude::ZeroCopySend;

#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(ZeroCopySend)]
#[repr(C)]
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

impl<const N: usize> Display for LogBuffer<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        // Get a safe reference to the bytes
        let bytes_ptr = self.bytes.as_ptr() as *const u8;
        let byte_slice = unsafe { core::slice::from_raw_parts(bytes_ptr, self.length) };

        // Convert bytes to a string, handling invalid UTF-8
        match core::str::from_utf8(byte_slice) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => {
                write!(f, "[invalid utf-8]")
                // TODO do this with some compiler flags that check if std feature is available or something
                // // Handle invalid UTF-8 by creating a lossy conversion
                // let lossy_string = String::from_utf8_lossy(byte_slice);
                // write!(f, "{}", lossy_string)
            }
        }
    }
}
