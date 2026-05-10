use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::{
    CoreError,
    entities::{Job, JobStatus, Meeting, Transcript, Segment},
    ports::{AudioCapture, CaptureSource, JobRepo, LlmProvider, MeetingFileStore, MeetingRepo, TemplateLoader, Transcriber},
};

// ── FakeTranscriber ──────────────────────────────────────────────────────────

pub struct FakeTranscriber {
    text: String,
}

impl FakeTranscriber {
    pub fn new(text: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { text: text.into() })
    }
}

#[async_trait]
impl Transcriber for FakeTranscriber {
    async fn transcribe(&self, _audio_path: &Path) -> Result<Transcript, CoreError> {
        Ok(Transcript {
            text: self.text.clone(),
            segments: vec![Segment { start_ms: 0, end_ms: 1000, text: self.text.clone() }],
            language: "ru".to_string(),
        })
    }
}

// ── FakeMeetingRepo ──────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeMeetingRepo {
    store: Mutex<Vec<Meeting>>,
}

impl FakeMeetingRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl MeetingRepo for FakeMeetingRepo {
    async fn save(&self, meeting: &Meeting) -> Result<(), CoreError> {
        self.store.lock().unwrap().push(meeting.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Meeting>, CoreError> {
        Ok(self.store.lock().unwrap().iter().find(|m| m.id == id).cloned())
    }

    async fn find_by_audio_path(&self, path: &Path) -> Result<Option<Meeting>, CoreError> {
        Ok(self.store.lock().unwrap().iter()
            .filter(|m| m.audio_path == PathBuf::from(path))
            .last()
            .cloned())
    }

    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = Some(text.to_string());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_protocol(&self, id: &str, text: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.protocol_text = Some(text.to_string());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_transcript_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = Some(text.to_string());
            m.transcript_path = Some(path.to_path_buf());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn save_protocol_file(&self, id: &str, text: &str, path: &Path) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.protocol_text = Some(text.to_string());
            m.protocol_path = Some(path.to_path_buf());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn update_name(&self, id: &str, name: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.name = name.to_string();
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
    }

    async fn list_all(&self) -> Result<Vec<Meeting>, CoreError> {
        Ok(self.store.lock().unwrap().clone())
    }
}

// ── FakeMeetingFileStore ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeMeetingFileStore {
    pub written: Mutex<Vec<(PathBuf, String)>>,
}

impl FakeMeetingFileStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl MeetingFileStore for FakeMeetingFileStore {
    async fn write_transcript(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        let path = dir.join("transcript.md");
        self.written.lock().unwrap().push((path.clone(), text.to_string()));
        Ok(path)
    }

    async fn write_protocol(&self, dir: &Path, text: &str) -> Result<PathBuf, CoreError> {
        let path = dir.join("protocol.md");
        self.written.lock().unwrap().push((path.clone(), text.to_string()));
        Ok(path)
    }
}

// ── FakeJobRepo ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeJobRepo {
    store: Mutex<Vec<Job>>,
}

impl FakeJobRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl JobRepo for FakeJobRepo {
    async fn enqueue(&self, job: &Job) -> Result<(), CoreError> {
        self.store.lock().unwrap().push(job.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Job>, CoreError> {
        Ok(self.store.lock().unwrap().iter().find(|j| j.id == id).cloned())
    }

    async fn claim_pending(&self, now_ts: i64) -> Result<Option<Job>, CoreError> {
        let mut store = self.store.lock().unwrap();
        let idx = store
            .iter()
            .position(|j| j.status == JobStatus::Pending && j.retry_after <= now_ts);
        if let Some(i) = idx {
            store[i].status = JobStatus::Running;
            store[i].updated_at = now_ts;
            Ok(Some(store[i].clone()))
        } else {
            Ok(None)
        }
    }

    async fn mark_done(&self, id: &str, now_ts: i64) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Done;
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn reset_for_retry(
        &self,
        id: &str,
        error: &str,
        attempts: u32,
        retry_after: i64,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Pending;
            j.attempts = attempts;
            j.last_error = Some(error.to_string());
            j.retry_after = retry_after;
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn mark_permanently_failed(
        &self,
        id: &str,
        error: &str,
        attempts: u32,
        now_ts: i64,
    ) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(j) = store.iter_mut().find(|j| j.id == id) {
            j.status = JobStatus::Failed;
            j.attempts = attempts;
            j.last_error = Some(error.to_string());
            j.updated_at = now_ts;
        }
        Ok(())
    }

    async fn recover_running_jobs(&self, now_ts: i64) -> Result<u64, CoreError> {
        let mut store = self.store.lock().unwrap();
        let mut count = 0u64;
        for j in store.iter_mut().filter(|j| j.status == JobStatus::Running) {
            j.status = JobStatus::Pending;
            j.attempts += 1;
            j.last_error = Some("interrupted by process restart".into());
            j.updated_at = now_ts;
            count += 1;
        }
        Ok(count)
    }
}

// ── FakeLlmProvider ──────────────────────────────────────────────────────────

pub struct FakeLlmProvider {
    response: String,
}

impl FakeLlmProvider {
    pub fn new(response: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { response: response.into() })
    }
}

#[async_trait]
impl LlmProvider for FakeLlmProvider {
    async fn generate(&self, _transcript: &str, _instructions: Option<&str>) -> Result<String, CoreError> {
        Ok(self.response.clone())
    }
}

// ── FakeTemplateLoader ───────────────────────────────────────────────────────

pub struct FakeTemplateLoader {
    templates: HashMap<String, String>,
}

impl FakeTemplateLoader {
    pub fn new(templates: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Arc<Self> {
        Arc::new(Self {
            templates: templates.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        })
    }

    pub fn empty() -> Arc<Self> {
        Arc::new(Self { templates: HashMap::new() })
    }
}

#[async_trait]
impl TemplateLoader for FakeTemplateLoader {
    async fn load(&self, name: &str) -> Result<Option<String>, CoreError> {
        Ok(self.templates.get(name).cloned())
    }

    async fn list_names(&self) -> Result<Vec<String>, CoreError> {
        Ok(self.templates.keys().cloned().collect())
    }
}

// ── FakeAudioCapture ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeAudioCapture {
    active: Mutex<HashSet<String>>,
    /// Each entry is `(session_id, source, echo_cancel)` in call order.
    pub started: Mutex<Vec<(String, CaptureSource, bool)>>,
    pub stopped: Mutex<Vec<String>>,
}

impl FakeAudioCapture {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Returns the `CaptureSource` for the most-recently started session, if any.
    pub fn last_source(&self) -> Option<CaptureSource> {
        self.started.lock().unwrap().last().map(|(_, s, _)| *s)
    }

    /// Returns the `echo_cancel` flag for the most-recently started session, if any.
    pub fn last_echo_cancel(&self) -> Option<bool> {
        self.started.lock().unwrap().last().map(|(_, _, e)| *e)
    }
}

#[async_trait]
impl AudioCapture for FakeAudioCapture {
    async fn start_session(
        &self,
        session_id: &str,
        _output_path: &Path,
        source: CaptureSource,
        echo_cancel: bool,
    ) -> Result<(), CoreError> {
        self.active.lock().unwrap().insert(session_id.to_string());
        self.started.lock().unwrap().push((session_id.to_string(), source, echo_cancel));
        Ok(())
    }

    async fn stop_session(&self, session_id: &str) -> Result<(), CoreError> {
        let removed = self.active.lock().unwrap().remove(session_id);
        if !removed {
            return Err(CoreError::Recording(format!("session not found: {session_id}")));
        }
        self.stopped.lock().unwrap().push(session_id.to_string());
        Ok(())
    }

    fn is_active(&self, session_id: &str) -> bool {
        self.active.lock().unwrap().contains(session_id)
    }
}
