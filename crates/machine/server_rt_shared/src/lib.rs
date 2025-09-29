#![no_std]

use iceoryx2::prelude::ZeroCopySend;
use crate::log::LogBuffer;

pub mod log;
pub mod sendable_ptr;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum IoStatus {
    Pending,
    Ready,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(ZeroCopySend)]
#[repr(C)]
pub enum RtResponse {
    Pong,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(ZeroCopySend)]
#[repr(C)]
pub enum MainRequest {
    Ping,
    RequestShutdown,
    EnableIo,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(ZeroCopySend)]
#[repr(C)]
pub enum RtRequest<const MAX_LOG_LENGTH: usize> {
    Log(LogBuffer<MAX_LOG_LENGTH>),
    Shutdown,
    StabilityChanged(StabilizationStatus),
}

#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(ZeroCopySend)]
#[repr(C)]
pub enum MainResponse {
    None,
    Ack,
    Nack,
}

#[derive(Debug)]
#[derive(Copy, Clone, PartialEq)]
#[derive(ZeroCopySend)]
#[repr(C)]
pub enum StabilizationStatus {
    Stable,
    Unstable,
}
