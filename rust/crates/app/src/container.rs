use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Context, Result};
use meeting_core::ports::{AudioCapture, JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber};
use meeting_adapters::{
    AnthropicProvider, CpalAudioCapture, Db, FileTemplateLoader, FsMeetingFileStore,
    LazyWhisperTranscriber, SqliteJobRepo, SqliteMeetingRepo, TranscriberPrefs,
    WhisperTranscriber, Worker,
};

pub struct Container {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
    pub audio_capture: Arc<dyn AudioCapture>,
    pub recordings_dir: PathBuf,
}

impl Container {
    pub fn new_desktop(
        model_path: &Path,
        db_path: &Path,
        prompts_dir: &Path,
        recordings_dir: PathBuf,
    ) -> Result<Self> {
        let transcriber = Arc::new(WhisperTranscriber::new(model_path)?);
        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY is not set")?;
        let llm = Arc::new(AnthropicProvider::new(api_key));
        let templates = Arc::new(FileTemplateLoader::new(prompts_dir));
        let audio_capture = Arc::new(CpalAudioCapture::new());

        Ok(Self { transcriber, meeting_repo, job_repo, llm, templates, audio_capture, recordings_dir })
    }

    /// Sidecar wiring — mirrors the `ffi/app_core.rs` adapter graph rather than
    /// [`Container::new_desktop`]:
    ///
    /// - **Lazy** Whisper transcriber (model loaded on first transcription, not
    ///   at boot) so the sidecar comes up fast and the stdout handshake is not
    ///   blocked on a multi-hundred-MB model load.
    /// - **Tolerant of a missing `ANTHROPIC_API_KEY`** — protocol generation
    ///   fails later with a clear error instead of preventing the whole sidecar
    ///   (recording, transcription, listing) from ever starting.
    ///
    /// `new_desktop` (used by the legacy `Serve` subcommand) is left unchanged.
    pub fn new_sidecar(
        model_path: &Path,
        db_path: &Path,
        prompts_dir: &Path,
        recordings_dir: PathBuf,
    ) -> Result<Self> {
        let prefs = TranscriberPrefs::new("ru", 1, 0);
        let transcriber: Arc<dyn Transcriber> =
            Arc::new(LazyWhisperTranscriber::new(model_path.to_path_buf(), prefs));

        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        // Missing key is non-fatal here (unlike `new_desktop`): the sidecar must
        // still serve recording/transcription/listing without it.
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        let llm = Arc::new(AnthropicProvider::new(api_key));
        let templates = Arc::new(FileTemplateLoader::new(prompts_dir));
        let audio_capture = Arc::new(CpalAudioCapture::new());

        Ok(Self {
            transcriber,
            meeting_repo,
            job_repo,
            llm,
            templates,
            audio_capture,
            recordings_dir,
        })
    }

    /// Spawn the background worker. Returns the join handle and a sender to request
    /// graceful shutdown (worker finishes current job then exits).
    pub fn spawn_worker(
        &self,
    ) -> (tokio::task::JoinHandle<()>, tokio::sync::oneshot::Sender<()>) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let worker = Worker::new(
            Arc::clone(&self.job_repo),
            Arc::clone(&self.meeting_repo),
            Arc::clone(&self.transcriber),
            Arc::new(FsMeetingFileStore),
        );
        let handle = tokio::spawn(worker.run(shutdown_rx));
        (handle, shutdown_tx)
    }
}

pub fn default_model_path() -> PathBuf {
    xdg_data_dir().join("meeting-assistant/models/ggml-medium.bin")
}

pub fn default_db_path() -> PathBuf {
    // Separate from Python's index.db to avoid schema conflicts during transition.
    xdg_data_dir().join("meeting-assistant/rust-index.db")
}

pub fn default_recordings_dir() -> PathBuf {
    xdg_cache_dir().join("meeting-assistant/recordings")
}

pub fn default_prompts_dir() -> PathBuf {
    // Shared with Python: repo-root/prompts/
    // Walk up from the binary location or fall back to CWD/prompts
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            // <repo>/rust/target/debug/meeting-assistant → <repo>/prompts
            p.ancestors().nth(4).map(|root| root.join("prompts"))
        })
        .unwrap_or_else(|| PathBuf::from("prompts"))
}

fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            PathBuf::from(home).join(".local/share")
        })
}

fn xdg_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            PathBuf::from(home).join(".cache")
        })
}
