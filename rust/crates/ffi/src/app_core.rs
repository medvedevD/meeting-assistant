use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use parking_lot::Mutex;

use meeting_core::ports::{AudioCapture, JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber};
use meeting_adapters::{
    AnthropicProvider, CpalAudioCapture, Db, FileTemplateLoader, FsMeetingFileStore,
    JsonSettingsStore, LazyWhisperTranscriber, SqliteJobRepo, SqliteMeetingRepo,
    TranscriberPrefs,
};
use crate::types::{AppConfig, AppError, FfiResult};

const APP_DIRNAME: &str = "meeting-assistant";

// ── Ring log buffer ───────────────────────────────────────────────────────────

pub(crate) type LogBuffer = Arc<Mutex<VecDeque<String>>>;

pub(crate) struct RingWriter {
    pub buffer: LogBuffer,
    pub capacity: usize,
}

impl std::io::Write for RingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        let line = s.trim_end_matches(['\n', '\r']);
        if !line.is_empty() {
            let mut lock = self.buffer.lock();
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(line.to_owned());
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

pub(crate) struct RingMakeWriter {
    pub buffer: LogBuffer,
    pub capacity: usize,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RingMakeWriter {
    type Writer = RingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        RingWriter { buffer: Arc::clone(&self.buffer), capacity: self.capacity }
    }
}

// ── AppCore ───────────────────────────────────────────────────────────────────

/// Central application object. Created once by Kotlin, passed to all FFI calls.
#[derive(uniffi::Object)]
pub struct AppCore {
    pub(crate) transcriber:      Arc<dyn Transcriber>,
    /// Direct reference to the lazy transcriber for runtime prefs updates.
    pub(crate) lazy_transcriber: Arc<LazyWhisperTranscriber>,
    pub(crate) meeting_repo:     Arc<dyn MeetingRepo>,
    pub(crate) audio_capture:    Arc<dyn AudioCapture>,
    pub(crate) job_repo:         Arc<dyn JobRepo>,
    pub(crate) llm:              Arc<dyn LlmProvider>,
    pub(crate) templates:        Arc<dyn TemplateLoader>,
    pub(crate) settings:         Arc<JsonSettingsStore>,
    pub(crate) file_store:       Arc<dyn MeetingFileStore>,
    pub(crate) meetings_dir:     PathBuf,
    pub(crate) model_path:       PathBuf,
    pub(crate) db_path:          PathBuf,
    pub(crate) prompts_dir:      PathBuf,
    pub(crate) log_buffer:       LogBuffer,
}

// ── WorkerHandle ──────────────────────────────────────────────────────────────

/// Handle returned by `start_worker`.
///
/// Prefer `stop_graceful()` over `stop()` — it lets the worker finish its
/// current job before exiting, and only falls back to an immediate abort if the
/// given timeout is exceeded.
#[derive(uniffi::Object)]
pub struct WorkerHandle {
    /// Signals the worker loop to exit cleanly after its current task.
    shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Owned join handle — taken when we await completion.
    join_handle: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Fallback for immediate abort.
    abort_handle: tokio::task::AbortHandle,
}

#[uniffi::export]
impl WorkerHandle {
    /// Immediate, forceful abort. The worker may be mid-job — prefer `stop_graceful`.
    pub fn stop(&self) {
        self.abort_handle.abort();
    }

    /// Returns true if the worker task has already finished (completed or aborted).
    pub fn is_finished(&self) -> bool {
        self.abort_handle.is_finished()
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl WorkerHandle {
    /// Signal the worker to stop after its current job, then wait up to
    /// `timeout_ms` milliseconds. Falls back to `stop()` if the timeout expires.
    pub async fn stop_graceful(&self, timeout_ms: u64) {
        // Take values out of mutexes before any await point.
        let tx     = self.shutdown_tx.lock().take();
        let handle = self.join_handle.lock().take();

        if let Some(tx) = tx {
            let _ = tx.send(());
        }
        if let Some(h) = handle {
            let dur = tokio::time::Duration::from_millis(timeout_ms);
            if tokio::time::timeout(dur, h).await.is_err() {
                tracing::warn!("worker did not finish within {timeout_ms}ms — aborting");
                self.abort_handle.abort();
            }
        }
    }
}

// ── Singleton lockfile ────────────────────────────────────────────────────────

/// Holds the open lockfile for the lifetime of the process.
/// The OS releases the flock automatically when the process exits (including kill -9).
static SINGLETON_LOCK_FILE: OnceLock<std::fs::File> = OnceLock::new();

/// Attempt to acquire a single-instance lock for the application.
///
/// Uses an exclusive advisory flock on
/// `$XDG_DATA_HOME/meeting-assistant/meeting-assistant.lock`.
/// The lock is held for the lifetime of the process — the OS releases it on exit,
/// even after kill -9, so stale lockfiles are never a problem.
///
/// Returns `Err(AppError::General)` if another live instance already holds the lock.
#[uniffi::export]
pub fn try_acquire_singleton() -> FfiResult<()> {
    use fs2::FileExt;

    if SINGLETON_LOCK_FILE.get().is_some() {
        return Ok(());
    }

    let lock_path = xdg_data_dir().join("meeting-assistant/meeting-assistant.lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::general)?;
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock_path)
        .map_err(AppError::general)?;

    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = SINGLETON_LOCK_FILE.set(file);
            Ok(())
        }
        Err(_) => Err(AppError::General(
            "Another instance of Meeting Assistant is already running.".into(),
        )),
    }
}

// ── Public FFI functions ──────────────────────────────────────────────────────

/// Initialize the application core.
///
/// Priority order for each path/key:
///   1. `config` parameter (explicit override from Kotlin)
///   2. `settings.json` on disk (persisted by the app)
///   3. Environment variable (`MEETING_ASSISTANT_MODEL`, etc.)
///   4. XDG default
///
/// Initialises the application core. The Whisper model is loaded lazily on first transcription.
#[uniffi::export]
pub fn init_core(config: AppConfig) -> Arc<AppCore> {
    let log_buffer: LogBuffer = Arc::new(Mutex::new(VecDeque::with_capacity(500)));

    let ring = RingMakeWriter { buffer: Arc::clone(&log_buffer), capacity: 500 };
    let _ = tracing_subscriber::fmt()
        .with_writer(ring)
        .with_ansi(false)
        .try_init();

    let settings = Arc::new(JsonSettingsStore::open_default());
    let s = settings.load();

    // Resolve API key: config override → stored → env
    let api_key = config.anthropic_api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| s.anthropic_api_key.clone().filter(|k| !k.is_empty()))
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());

    if let Some(key) = &api_key {
        // SAFETY: called once at startup before other threads read the env.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", key); }
    }

    let model_path = resolve_path(
        config.model_path.as_deref(),
        s.paths.model.as_deref(),
        "MEETING_ASSISTANT_MODEL",
        || xdg_data_dir().join("meeting-assistant/models/ggml-medium.bin"),
    );
    let db_path = resolve_path(
        config.db_path.as_deref(),
        s.paths.db.as_deref(),
        "MEETING_ASSISTANT_DB",
        || xdg_data_dir().join("meeting-assistant/rust-index.db"),
    );
    let meetings_dir = resolve_path(
        config.meetings_dir.as_deref(),
        s.paths.meetings_dir.as_deref(),
        "MEETING_ASSISTANT_MEETINGS_DIR",
        || xdg_documents_dir().join(APP_DIRNAME),
    );
    let prompts_dir = resolve_path(
        config.prompts_dir.as_deref(),
        s.paths.prompts.as_deref(),
        "MEETING_ASSISTANT_PROMPTS",
        default_prompts_dir,
    );

    let prefs = TranscriberPrefs::new(s.transcriber.language.clone(), s.transcriber.beam_size, s.transcriber.n_threads);
    let lazy_transcriber = Arc::new(LazyWhisperTranscriber::new(model_path.clone(), prefs));
    let transcriber: Arc<dyn Transcriber> = Arc::clone(&lazy_transcriber) as Arc<dyn Transcriber>;

    let db = Db::open(&db_path).expect("failed to open database");
    let meeting_repo: Arc<dyn MeetingRepo> = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
    let job_repo: Arc<dyn JobRepo>          = Arc::new(SqliteJobRepo(Arc::clone(&db)));

    let llm: Arc<dyn LlmProvider>           = Arc::new(AnthropicProvider::new(api_key.unwrap_or_default()));
    let templates: Arc<dyn TemplateLoader>  = Arc::new(FileTemplateLoader::new(&prompts_dir));
    let audio_capture: Arc<dyn AudioCapture> = Arc::new(CpalAudioCapture::new());
    let file_store: Arc<dyn MeetingFileStore> = Arc::new(FsMeetingFileStore);

    Arc::new(AppCore {
        transcriber,
        lazy_transcriber,
        meeting_repo,
        audio_capture,
        job_repo,
        llm,
        templates,
        settings,
        file_store,
        meetings_dir,
        model_path,
        db_path,
        prompts_dir,
        log_buffer,
    })
}

/// Start the background worker that processes transcription jobs from the DB queue.
/// Returns a `WorkerHandle`. Call `stop_graceful()` on shutdown for clean teardown.
///
/// Also triggers warm preload of the Whisper model so the first transcription has no cold start.
#[uniffi::export(async_runtime = "tokio")]
pub async fn start_worker(core: Arc<AppCore>) -> Arc<WorkerHandle> {
    // Warm preload runs concurrently with worker startup — non-blocking.
    let lazy = Arc::clone(&core.lazy_transcriber);
    tokio::spawn(async move {
        if let Err(e) = lazy.ensure_loaded().await {
            tracing::warn!("warm preload failed: {e}");
        }
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker = meeting_adapters::Worker::new(
        Arc::clone(&core.job_repo),
        Arc::clone(&core.meeting_repo),
        Arc::clone(&core.transcriber),
        Arc::clone(&core.file_store),
    );
    let join_handle = tokio::spawn(worker.run(shutdown_rx));
    let abort_handle = join_handle.abort_handle();
    Arc::new(WorkerHandle {
        shutdown_tx: parking_lot::Mutex::new(Some(shutdown_tx)),
        join_handle:  parking_lot::Mutex::new(Some(join_handle)),
        abort_handle,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolves a path with the priority: explicit override → persisted setting → env var → default.
fn resolve_path(
    override_val: Option<&str>,
    stored: Option<&str>,
    env_key: &str,
    default: impl FnOnce() -> PathBuf,
) -> PathBuf {
    override_val
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| stored.filter(|p| !p.trim().is_empty()).map(PathBuf::from))
        .or_else(|| std::env::var_os(env_key).map(PathBuf::from))
        .unwrap_or_else(default)
}

fn xdg_documents_dir() -> PathBuf {
    std::process::Command::new("xdg-user-dir")
        .arg("DOCUMENTS")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Documents")
        })
}

pub(crate) fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        })
}

fn default_prompts_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.ancestors().nth(4).map(|root| root.join("prompts")))
        .unwrap_or_else(|| PathBuf::from("prompts"))
}
