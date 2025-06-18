/// You can enable trace logging by setting the RUST_LOG environment variable to "trace", e.g. "RUST_LOG=trace"
/// See: https://docs.rs/env_logger/0.9.1/env_logger/#capturing-logs-in-tests
///
/// Requires "env_logger" feature.
pub fn logging_init() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();
}
