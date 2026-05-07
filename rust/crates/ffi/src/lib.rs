uniffi::setup_scaffolding!();

pub mod types;
pub mod app_core;
pub mod recording;
pub mod meetings;
pub mod transcription;
pub mod templates;
pub mod settings;
pub mod diagnostics;

mod tests;

// Re-export the public API surface so Kotlin bindings pick everything up.
pub use types::*;
pub use app_core::{AppCore, init_core, start_worker};
