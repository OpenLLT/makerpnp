#![no_std]

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
pub enum Message<REQ: Copy + Clone, RES: Copy + Clone> {
    Request(Request<REQ>),
    // request reference, response payload
    Response(Response<RES>),
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub struct Request<T: Copy + Clone> {
    pub index: usize,
    pub payload: T,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub struct Response<T: Copy + Clone> {
    pub request_reference: usize,
    pub payload: T,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum RtResponse {
    Pong,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum MainRequest {
    Ping,
    RequestShutdown,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum RtRequest<const MAX_LOG_LENGTH: usize> {
    Log(LogBuffer<MAX_LOG_LENGTH>),
    Shutdown,
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum MainResponse {
    None,
}
