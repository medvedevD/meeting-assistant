//! Compiles iff `meeting_api::LiveProgress` and `meeting_adapters::LiveProgress`
//! resolve to the same nominal type (the one defined in `meeting_core`).
//! Guards against accidental re-introduction of parallel type aliases.

#[allow(dead_code)]
fn api_to_adapters(p: meeting_api::LiveProgress) -> meeting_adapters::LiveProgress {
    p
}

#[allow(dead_code)]
fn core_to_api(p: meeting_core::LiveProgress) -> meeting_api::LiveProgress {
    p
}

#[allow(dead_code)]
fn core_to_adapters(p: meeting_core::LiveProgress) -> meeting_adapters::LiveProgress {
    p
}
