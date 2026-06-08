/// A read-only set of prompt templates shipped with the application.
///
/// The implementation in production embeds the templates in the binary at
/// compile time; tests use an in-memory fake. The use-case
/// [`crate::usecases::backfill_templates`] consumes this port to seed any
/// bundled template missing from the writable prompts dir, so core never sees
/// `include_str!` or the filesystem.
pub trait TemplateBundle: Send + Sync {
    /// `(name, body)` for every bundled template. `name` is the bare template
    /// name (no extension, no path separators).
    fn entries(&self) -> Vec<(String, String)>;
}
