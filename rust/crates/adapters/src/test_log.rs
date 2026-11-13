//! Captures what the sidecar's `fmt` logger would write, for tests that check
//! a log line.

use tracing_subscriber::EnvFilter;

#[derive(Clone, Default)]
pub(crate) struct LogBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl LogBuf {
    /// Route this thread's events at `filter` here until the guard drops.
    /// Events from other threads (a multi-thread runtime's workers) are not
    /// captured.
    pub(crate) fn capture(&self, filter: &str) -> tracing::subscriber::DefaultGuard {
        let buf = self.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || buf.clone())
            .with_ansi(false)
            .with_env_filter(EnvFilter::new(filter))
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }

    /// The first line containing `needle`; panics with the whole log if none.
    pub(crate) fn line_with(&self, needle: &str) -> String {
        let text = self.text();
        text.lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no {needle:?} in the log:\n{text}"))
            .to_string()
    }
}
