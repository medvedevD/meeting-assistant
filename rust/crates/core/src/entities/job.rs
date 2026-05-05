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
    Transcribe,
}

impl JobKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Transcribe => "transcribe",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "transcribe" => Some(Self::Transcribe),
            _ => None,
        }
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
    pub created_at: i64,
    pub updated_at: i64,
}

impl Job {
    pub fn new_transcribe(meeting_id: String) -> Self {
        let now = now_unix();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id,
            kind: JobKind::Transcribe,
            status: JobStatus::Pending,
            attempts: 0,
            last_error: None,
            retry_after: 0,
            created_at: now,
            updated_at: now,
        }
    }
}
