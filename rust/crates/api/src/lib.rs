mod router;
mod routes;
mod settings_service;
mod template_service;

pub use router::{
    create_router, create_server_router, no_default_template, AppState, DefaultTemplateFn,
    LiveProgress, VersionInfo,
};
pub use settings_service::SettingsService;
pub use template_service::{TemplateDto, TemplateError, TemplateService};

/// IPC protocol version negotiated between the Qt GUI client and this server.
///
/// **Single source of truth.** Bumped ONLY on a breaking IPC change (a route
/// removed, or request/response semantics changed incompatibly). Additive
/// changes — a new route, a new optional field — do NOT bump this. The GUI
/// proceeds iff its protocol version is within
/// `[MIN_PROTOCOL_VERSION ..= PROTOCOL_VERSION]`; it hard-fails only on a
/// breaking mismatch.
///
/// The C++/Qt client constant is generated from this value at build time
/// (section-03) or guarded by a CI equality check (section-08); never define it
/// twice by hand.
pub const PROTOCOL_VERSION: u32 = 1;

/// Oldest protocol version this server can still serve a client speaking.
pub const MIN_PROTOCOL_VERSION: u32 = 1;
