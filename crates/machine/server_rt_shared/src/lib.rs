#![no_std]

pub mod sendable_ptr;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum IoStatus {
    Pending,
    Ready,
}
