use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Context, Result};
use meeting_core::ports::{
    AudioCapture, JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber,
};
use meeting_adapters::{
    build_llm, AnthropicProvider, CpalAudioCapture, Db, FileTemplateLoader, FsMeetingFileStore,
    JsonSettingsStore, KeyringSecretStore, LazyWhisperTranscriber, LiveProgress, SqliteJobRepo,
    SqliteMeetingRepo, SwappableLlm, TranscriberPrefs, WhisperTranscriber, Worker,
};

/// Concrete handles the composition layer keeps so it can apply settings
/// changes at runtime (hot-swap LLM, update transcriber prefs/model, swap
/// prompts dir). Populated only by [`Container::new_sidecar`].
pub struct SettingsHandles {
    pub settings_store: Arc<JsonSettingsStore>,
    pub secrets: Arc<KeyringSecretStore>,
    pub llm: Arc<SwappableLlm>,
    pub transcriber: Arc<LazyWhisperTranscriber>,
    pub templates: Arc<FileTemplateLoader>,
}

pub struct Container {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
    pub audio_capture: Arc<dyn AudioCapture>,
    pub file_store: Arc<dyn MeetingFileStore>,
    pub recordings_dir: PathBuf,
    /// Live, in-memory job-progress table shared between the worker (writer)
    /// and the `GET /jobs/:id` handler (reader). Never persisted.
    pub progress: LiveProgress,
    /// `None` in CLI (`new_desktop`) mode; `Some` for the sidecar.
    pub settings_handles: Option<SettingsHandles>,
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
        let file_store: Arc<dyn MeetingFileStore> = Arc::new(FsMeetingFileStore);

        Ok(Self { transcriber, meeting_repo, job_repo, llm, templates, audio_capture, file_store, recordings_dir, progress: Arc::new(dashmap::DashMap::new()), settings_handles: None })
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
        // ── Settings + secrets are the single source of configuration ─────────
        let settings_store = Arc::new(JsonSettingsStore::open_default());
        let secrets = Arc::new(KeyringSecretStore::open_default());

        // One-time migration: a pre-keyring build stored the Anthropic key in
        // plaintext inside settings.json. Move it into the keyring, then clear it.
        let mut settings = settings_store.load();
        if let Some(key) = settings.anthropic_api_key.take() {
            if !key.is_empty() {
                let _ = secrets.set("anthropic", &key);
                tracing::info!("migrated legacy plaintext Anthropic key into the keyring");
            }
            let _ = settings_store.save(settings.clone());
        }

        // Effective paths: a settings override wins over the passed default. (db
        // and recordings overrides are restart-required and applied at boot.)
        let model = settings.transcriber.model_path.clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| model_path.to_path_buf());
        let prompts = settings.paths.prompts.clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| prompts_dir.to_path_buf());

        // ── Transcriber (lazy) with prefs from settings ───────────────────────
        let prefs = TranscriberPrefs::new(
            settings.transcriber.language.clone(),
            settings.transcriber.beam_size,
            settings.transcriber.n_threads,
        );
        let transcriber_handle = Arc::new(LazyWhisperTranscriber::new(model, prefs));
        let transcriber: Arc<dyn Transcriber> = transcriber_handle.clone();

        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        // ── LLM: active provider, with the effective key (env → keyring/file) ──
        let active = settings.llm.active;
        let key = secrets.effective_key(active.as_str());
        let cfg = settings.llm.resolve(active, key);
        let llm_handle = Arc::new(SwappableLlm::new(build_llm(&cfg)));
        let llm: Arc<dyn LlmProvider> = llm_handle.clone();

        let templates_handle = Arc::new(FileTemplateLoader::new(prompts));
        let templates: Arc<dyn TemplateLoader> = templates_handle.clone();

        let audio_capture = Arc::new(CpalAudioCapture::new());
        let file_store: Arc<dyn MeetingFileStore> = Arc::new(FsMeetingFileStore);

        Ok(Self {
            transcriber,
            meeting_repo,
            job_repo,
            llm,
            templates,
            audio_capture,
            file_store,
            recordings_dir,
            progress: Arc::new(dashmap::DashMap::new()),
            settings_handles: Some(SettingsHandles {
                settings_store,
                secrets,
                llm: llm_handle,
                transcriber: transcriber_handle,
                templates: templates_handle,
            }),
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
            Arc::clone(&self.file_store),
            Arc::clone(&self.llm),
            Arc::clone(&self.templates),
            Arc::clone(&self.progress),
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
    // User-visible content → OS Documents folder, not a cache dir.
    // `dirs::document_dir()` resolves XDG user-dirs on Linux, `$HOME/Documents` on macOS,
    // and `FOLDERID_Documents` on Windows (OneDrive-redirect aware).
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Documents")))
        .expect("cannot resolve user's Documents directory")
        .join("meeting-assistant")
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

