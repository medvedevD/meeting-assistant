use serde::{Deserialize, Serialize};
use super::meeting::now_unix;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// First-time transcription of a freshly recorded/imported meeting.
    Transcribe,
    /// Re-run transcription on an existing meeting (transcript + protocol were
    /// cleared by the `reprocess_transcribe` use-case before enqueue).
    ReprocessTranscribe,
    /// Regenerate the protocol from the stored transcript, optionally with a
    /// different template (carried in `Job::template_name`).
    RegenerateProtocol,
}

impl JobKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Transcribe => "transcribe",
            Self::ReprocessTranscribe => "reprocess_transcribe",
            Self::RegenerateProtocol => "regenerate_protocol",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "transcribe" => Some(Self::Transcribe),
            "reprocess_transcribe" => Some(Self::ReprocessTranscribe),
            "regenerate_protocol" => Some(Self::RegenerateProtocol),
            _ => None,
        }
    }

    /// True for kinds whose worker step runs the transcriber.
    pub fn is_transcription(&self) -> bool {
        matches!(self, Self::Transcribe | Self::ReprocessTranscribe)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub meeting_id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub attempts: u32,
    pub last_error: Option<String>,
    /// Unix timestamp; 0 means immediately eligible.
    pub retry_after: i64,
    /// Template to use for `RegenerateProtocol`; `None` falls back to the
    /// built-in default prompt. Ignored by transcription kinds.
    pub template_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Job {
    fn new(meeting_id: String, kind: JobKind, template_name: Option<String>) -> Self {
        let now = now_unix();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id,
            kind,
            status: JobStatus::Pending,
            attempts: 0,
            last_error: None,
            retry_after: 0,
            template_name,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_transcribe(meeting_id: String) -> Self {
        Self::new(meeting_id, JobKind::Transcribe, None)
    }

    pub fn new_reprocess_transcribe(meeting_id: String) -> Self {
        Self::new(meeting_id, JobKind::ReprocessTranscribe, None)
    }

    pub fn new_regenerate_protocol(meeting_id: String, template_name: Option<String>) -> Self {
        Self::new(meeting_id, JobKind::RegenerateProtocol, template_name)
    }
}
