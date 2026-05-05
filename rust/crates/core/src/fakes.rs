use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::{
    CoreError,
    entities::{Job, JobStatus, Meeting, Transcript, Segment},
    ports::{JobRepo, LlmProvider, MeetingRepo, TemplateLoader, Transcriber},
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

    async fn save_transcript(&self, id: &str, text: &str) -> Result<(), CoreError> {
        let mut store = self.store.lock().unwrap();
        if let Some(m) = store.iter_mut().find(|m| m.id == id) {
            m.transcript_text = Some(text.to_string());
            Ok(())
        } else {
            Err(CoreError::NotFound(id.to_string()))
        }
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
