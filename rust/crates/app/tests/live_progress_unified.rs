//! Compiles iff `meeting_api::LiveJobs` and `meeting_adapters::LiveJobs`
//! resolve to the same nominal type (the one defined in `meeting_core`).
//! Guards against accidental re-introduction of parallel type aliases.

#[allow(dead_code)]
fn api_to_adapters(p: meeting_api::LiveJobs) -> meeting_adapters::LiveJobs {
    p
}

#[allow(dead_code)]
fn core_to_api(p: meeting_core::LiveJobs) -> meeting_api::LiveJobs {
    p
}

#[allow(dead_code)]
fn core_to_adapters(p: meeting_core::LiveJobs) -> meeting_adapters::LiveJobs {
    p
}
