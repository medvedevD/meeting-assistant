use anyhow::{Context, Result};
use meeting_adapters::{
    build_llm, resolve_transcription_model_path, AnthropicProvider, CpalAudioCapture, Db,
    FileTemplateLoader, FsMeetingFileStore, JsonSettingsStore, KeyringSecretStore,
    LazyWhisperTranscriber, LiveJobs, SqliteJobRepo, SqliteMeetingRepo, SwappableLlm,
    TranscriberPrefs, WhisperTranscriber, Worker, PROTOCOL_KINDS, TRANSCRIBE_KINDS,
};
use meeting_core::entities::meeting::now_unix;
use meeting_core::ports::{
    AudioCapture, JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default cap on concurrent IO-bound protocol jobs. Protects against accidental
/// rate-limit storms (bulk-regen after a template change) while leaving room for
/// useful parallelism on the LLM provider.
const DEFAULT_IO_POOL: usize = 4;

/// Handles returned by [`Container::spawn_workers`] — one set per worker.
pub struct WorkerHandles {
    pub transcribe_join: tokio::task::JoinHandle<()>,
    pub transcribe_shutdown: tokio::sync::oneshot::Sender<()>,
    pub protocol_join: tokio::task::JoinHandle<()>,
    pub protocol_shutdown: tokio::sync::oneshot::Sender<()>,
}

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
    /// Live, in-memory job table shared between the worker (writer), the
    /// `GET /jobs/:id` handler (reader), and the `DELETE /jobs/:id` handler
    /// (cancellation-token signaller). Never persisted.
    pub progress: LiveJobs,
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

        let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set")?;
        let llm = Arc::new(AnthropicProvider::new(api_key));
        let templates = Arc::new(FileTemplateLoader::new(prompts_dir));
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
            progress: meeting_core::LiveProgress::new(),
            settings_handles: None,
        })
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
        let mut settings = settings_store.load().normalize();
        if let Some(key) = settings.anthropic_api_key.take() {
            if !key.is_empty() {
                let _ = secrets.set("anthropic", &key);
                tracing::info!("migrated legacy plaintext Anthropic key into the keyring");
            }
            let _ = settings_store.save(settings.clone());
        }

        // Effective paths: a settings override wins over the passed default. (db
        // and recordings overrides are restart-required and applied at boot.)
        let model_resolution = resolve_transcription_model_path(&settings);
        let model = model_resolution
            .as_ref()
            .ok()
            .cloned()
            .or_else(|| {
                model_resolution
                    .as_ref()
                    .err()
                    .and_then(|error| error.path.clone())
            })
            .unwrap_or_else(|| model_path.to_path_buf());
        let prompts = settings
            .paths
            .prompts
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| prompts_dir.to_path_buf());

        // ── Transcriber (lazy) with prefs from settings ───────────────────────
        let prefs = TranscriberPrefs::new(
            settings.transcriber.language.clone(),
            settings.transcriber.beam_size,
            settings.transcriber.n_threads,
            settings.transcriber.use_gpu,
        );
        let transcriber_handle = Arc::new(match model_resolution {
            Ok(_) => LazyWhisperTranscriber::new(model, prefs),
            Err(error) => LazyWhisperTranscriber::new_unavailable(model, error, prefs),
        });
        let transcriber: Arc<dyn Transcriber> = transcriber_handle.clone();

        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        // ── LLM: active provider, with the stored key (keyring/vault/file) ────
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
            progress: meeting_core::LiveProgress::new(),
            settings_handles: Some(SettingsHandles {
                settings_store,
                secrets,
                llm: llm_handle,
                transcriber: transcriber_handle,
                templates: templates_handle,
            }),
        })
    }

    /// Spawn the two background workers (transcribe + protocol) and run the
    /// one-shot crash recovery before either claims its first job. Returns
    /// per-worker join handles and shutdown senders.
    ///
    /// Pool sizing: `cpu_pool = max(1, num_cpus::get_physical() / max(1, threads_per_job))`
    /// where `threads_per_job` is `n_threads` from settings (0 → `num_cpus`,
    /// same convention as the Whisper inference path). `io_pool = 4`
    /// (rate-limit guard for the LLM provider). Both are fixed at startup —
    /// changing `n_threads` at runtime applies to new transcriptions but does
    /// not resize the pool (restart-required, consistent with `db_path`).
    pub async fn spawn_workers(&self) -> WorkerHandles {
        // One-shot crash recovery: reset `running` rows to `pending` for jobs
        // interrupted by the previous process. Best-effort: a failure here is
        // logged but does not block worker startup (the next iteration of
        // claim_pending_kind would just see no pending jobs and idle-sleep).
        match self.job_repo.recover_running_jobs(now_unix()).await {
            Ok(0) => {}
            Ok(n) => tracing::info!(count = n, "recovered interrupted jobs from previous run"),
            Err(e) => tracing::error!("recover_running_jobs failed: {e}"),
        }

        let threads_per_job = self
            .settings_handles
            .as_ref()
            .map(|h| h.settings_store.load().transcriber.n_threads as usize)
            .unwrap_or(0);
        let threads_per_job = if threads_per_job == 0 {
            num_cpus::get_physical()
        } else {
            threads_per_job
        };
        let cpu_pool = (num_cpus::get_physical() / threads_per_job.max(1)).max(1);
        let io_pool = DEFAULT_IO_POOL;
        tracing::info!(cpu_pool, io_pool, threads_per_job, "worker pools sized");

        let (transcribe_shutdown, t_rx) = tokio::sync::oneshot::channel::<()>();
        let transcribe_worker = Arc::new(Worker::new(
            Arc::clone(&self.job_repo),
            Arc::clone(&self.meeting_repo),
            Arc::clone(&self.transcriber),
            Arc::clone(&self.file_store),
            Arc::clone(&self.llm),
            Arc::clone(&self.templates),
            Arc::clone(&self.progress),
            TRANSCRIBE_KINDS,
            cpu_pool,
            "transcribe",
        ));
        let transcribe_join = tokio::spawn(transcribe_worker.run(t_rx));

        let (protocol_shutdown, p_rx) = tokio::sync::oneshot::channel::<()>();
        let protocol_worker = Arc::new(Worker::new(
            Arc::clone(&self.job_repo),
            Arc::clone(&self.meeting_repo),
            Arc::clone(&self.transcriber),
            Arc::clone(&self.file_store),
            Arc::clone(&self.llm),
            Arc::clone(&self.templates),
            Arc::clone(&self.progress),
            PROTOCOL_KINDS,
            io_pool,
            "protocol",
        ));
        let protocol_join = tokio::spawn(protocol_worker.run(p_rx));

        WorkerHandles {
            transcribe_join,
            transcribe_shutdown,
            protocol_join,
            protocol_shutdown,
        }
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
    // Writable, per-user, survives upgrades (like the db and models dirs). The
    // bundled templates are embedded in the binary and seeded here at startup by
    // `backfill_templates`; `settings.paths.prompts` still overrides this.
    xdg_data_dir().join("meeting-assistant/prompts")
}

fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(dirs::data_dir)
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
        .expect("cannot resolve user's data directory")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    struct EnvGuard {
        keys: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn clear(keys: &[&'static str]) -> Self {
            let keys = keys
                .iter()
                .map(|&key| {
                    let value = std::env::var_os(key);
                    std::env::remove_var(key);
                    (key, value)
                })
                .collect();
            Self { keys }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.keys.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn xdg_data_dir_does_not_require_home() {
        let _guard = EnvGuard::clear(&["XDG_DATA_HOME", "HOME"]);
        std::env::set_var("USERPROFILE", r"C:\Users\meeting-test");

        assert!(!xdg_data_dir().as_os_str().is_empty());
    }
}
