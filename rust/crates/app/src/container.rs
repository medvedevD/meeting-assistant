use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Context, Result};
use meeting_core::ports::{JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber};
use meeting_adapters::{AnthropicProvider, Db, FileTemplateLoader, SqliteJobRepo, SqliteMeetingRepo, WhisperTranscriber, Worker};

pub struct Container {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
    pub llm: Arc<dyn LlmProvider>,
    pub templates: Arc<dyn TemplateLoader>,
}

impl Container {
    pub fn new_desktop(model_path: &Path, db_path: &Path, prompts_dir: &Path) -> Result<Self> {
        let transcriber = Arc::new(WhisperTranscriber::new(model_path)?);
        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY is not set")?;
        let llm = Arc::new(AnthropicProvider::new(api_key));
        let templates = Arc::new(FileTemplateLoader::new(prompts_dir));

        Ok(Self { transcriber, meeting_repo, job_repo, llm, templates })
    }

    /// Spawn the background worker and return its join handle.
    pub fn spawn_worker(&self) -> tokio::task::JoinHandle<()> {
        let worker = Worker::new(
            Arc::clone(&self.job_repo),
            Arc::clone(&self.meeting_repo),
            Arc::clone(&self.transcriber),
        );
        tokio::spawn(worker.run())
    }
}

pub fn default_model_path() -> PathBuf {
    xdg_data_dir().join("meeting-assistant/models/ggml-medium.bin")
}

pub fn default_db_path() -> PathBuf {
    xdg_data_dir().join("meeting-assistant/index.db")
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
