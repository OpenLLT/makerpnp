// Define communication structures with careful memory layout for RT safety
#[repr(C)]
pub struct SharedState {
    latency_stats: [i32; 100],
}

impl SharedState {
    //
    // public api to be used from server
    //

    pub const fn new() -> Self {
        Self {
            latency_stats: [0; 100],
        }
    }

    /// Safety: no locking here, the values could be updated by the RT thread
    pub fn get_latency_stats(&self) -> &[i32; 100] {
        &self.latency_stats
    }

    //
    // private api, only to be used from this crate
    //

    pub(crate) const fn set_latency_stats(&mut self, latency_stats: [i32; 100]) {
        self.latency_stats = latency_stats;
    }
}
