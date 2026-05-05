use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use meeting_core::ports::{JobRepo, MeetingRepo, Transcriber};
use meeting_adapters::{Db, SqliteJobRepo, SqliteMeetingRepo, WhisperTranscriber, Worker};

pub struct Container {
    pub transcriber: Arc<dyn Transcriber>,
    pub meeting_repo: Arc<dyn MeetingRepo>,
    pub job_repo: Arc<dyn JobRepo>,
}

impl Container {
    pub fn new_desktop(model_path: &Path, db_path: &Path) -> Result<Self> {
        let transcriber = Arc::new(WhisperTranscriber::new(model_path)?);
        let db = Db::open(db_path)?;
        let meeting_repo = Arc::new(SqliteMeetingRepo(Arc::clone(&db)));
        let job_repo = Arc::new(SqliteJobRepo(Arc::clone(&db)));

        Ok(Self { transcriber, meeting_repo, job_repo })
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

fn xdg_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("$HOME is not set");
            PathBuf::from(home).join(".local/share")
        })
}
