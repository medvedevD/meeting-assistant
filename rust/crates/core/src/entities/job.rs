use super::meeting::now_unix;
use serde::{Deserialize, Serialize};

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

    pub fn parse(s: &str) -> Option<Self> {
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

    pub fn parse(s: &str) -> Option<Self> {
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

/// Coarse pipeline phase a job is currently in. Live-only (kept in memory,
/// never persisted) and surfaced via `GET /jobs/:id` while a job is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    Queued,
    LoadingModel,
    DecodingAudio,
    Transcribing,
    WritingTranscript,
    GeneratingProtocol,
    Done,
}

impl PipelineStage {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Queued => "queued",
            Self::LoadingModel => "loading_model",
            Self::DecodingAudio => "decoding_audio",
            Self::Transcribing => "transcribing",
            Self::WritingTranscript => "writing_transcript",
            Self::GeneratingProtocol => "generating_protocol",
            Self::Done => "done",
        }
    }
}

/// Classified terminal failure. This is the **only** progress-related value
/// persisted in the DB (decision #11); the stage/percent live in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    AudioCorrupt,
    ApiQuota,
    ApiAuth,
    NetworkTimeout,
    WorkerKilled,
    ModelNotSelected,
    ModelMissing,
    /// User-initiated cancellation via `DELETE /api/v1/jobs/:id`. Distinct
    /// from `WorkerKilled` (process-level termination) and from the retry
    /// path — see `plans/active/job-cancellation`.
    Cancelled,
    Unknown,
}

impl ErrorClass {
    pub fn as_str(&self) -> &str {
        match self {
            Self::AudioCorrupt => "audio_corrupt",
            Self::ApiQuota => "api_quota",
            Self::ApiAuth => "api_auth",
            Self::NetworkTimeout => "network_timeout",
            Self::WorkerKilled => "worker_killed",
            Self::ModelNotSelected => "model_not_selected",
            Self::ModelMissing => "model_missing",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "audio_corrupt" => Some(Self::AudioCorrupt),
            "api_quota" => Some(Self::ApiQuota),
            "api_auth" => Some(Self::ApiAuth),
            "network_timeout" => Some(Self::NetworkTimeout),
            "worker_killed" => Some(Self::WorkerKilled),
            "model_not_selected" => Some(Self::ModelNotSelected),
            "model_missing" => Some(Self::ModelMissing),
            "cancelled" => Some(Self::Cancelled),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Whether re-running the job could plausibly succeed. Transient faults
    /// (network blips, rate limits, a killed worker) are worth a backoff retry;
    /// configuration/content faults (bad or missing API key, corrupt audio, a
    /// model that isn't selected/installed) will fail identically every time, so
    /// the worker should surface them immediately instead of grinding retries.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NetworkTimeout | Self::ApiQuota | Self::WorkerKilled | Self::Unknown
        )
    }

    /// Map a terminal [`CoreError`] to a UI-facing error class. The LLM
    /// adapters already classify HTTP failures into the typed `Api*`/`Network`
    /// variants ([`crate::CoreError`]); transcription failures are inspected
    /// for the model-missing signature emitted by the Whisper loader.
    pub fn from_core_error(err: &crate::CoreError) -> Self {
        use crate::CoreError::*;
        match err {
            ApiAuth(_) => Self::ApiAuth,
            ApiQuota(_) => Self::ApiQuota,
            ApiTimeout(_) | Network(_) => Self::NetworkTimeout,
            AudioCorrupt(_) => Self::AudioCorrupt,
            WorkerKilled(_) => Self::WorkerKilled,
            Cancelled => Self::Cancelled,
            // The Whisper loader reports a model-load failure as a
            // `Transcription` error with this signature (RU/EN). Anything else
            // from transcription is an audio/decoding problem we can't pin down.
            Transcription(m) if is_model_not_selected(m) => Self::ModelNotSelected,
            Transcription(m) if is_model_missing(m) => Self::ModelMissing,
            _ => Self::Unknown,
        }
    }
}

fn is_model_not_selected(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("model_not_selected") || m.contains("model is not selected")
}

fn is_model_missing(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("model_missing")
        || m.contains("модель не найдена")
        || m.contains("model not found")
        || m.contains("failed to load whisper model")
}

/// Live progress for an active job. Held in memory only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub stage: PipelineStage,
    /// Short human-readable sub-status (RU), e.g. "Распознавание речи".
    pub sub: String,
    /// 0–100 within the current stage.
    pub percent: u8,
}

impl JobProgress {
    pub fn new(stage: PipelineStage, sub: impl Into<String>, percent: u8) -> Self {
        Self {
            stage,
            sub: sub.into(),
            percent,
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
    /// Template to use for `RegenerateProtocol`; `None` falls back to the
    /// built-in default prompt. For a transcription job with `then_protocol`,
    /// this carries the template for the protocol job enqueued on success.
    pub template_name: Option<String>,
    /// When set on a transcription job, the worker enqueues a protocol job once
    /// the transcript is written. Persists the "transcribe → protocol" chain so
    /// it survives a client/app restart. Always `false` for protocol jobs.
    pub then_protocol: bool,
    /// Classified terminal failure (persisted). `None` until the job fails
    /// permanently with a recognizable cause.
    pub error_class: Option<ErrorClass>,
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
            then_protocol: false,
            error_class: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoreError;

    #[test]
    fn error_class_roundtrips_through_str() {
        for c in [
            ErrorClass::AudioCorrupt,
            ErrorClass::ApiQuota,
            ErrorClass::ApiAuth,
            ErrorClass::NetworkTimeout,
            ErrorClass::WorkerKilled,
            ErrorClass::ModelNotSelected,
            ErrorClass::ModelMissing,
            ErrorClass::Cancelled,
            ErrorClass::Unknown,
        ] {
            assert_eq!(ErrorClass::parse(c.as_str()), Some(c));
        }
        assert_eq!(ErrorClass::parse("bogus"), None);
    }

    #[test]
    fn maps_core_errors_to_classes() {
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::ApiAuth("x".into())),
            ErrorClass::ApiAuth
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::ApiQuota("x".into())),
            ErrorClass::ApiQuota
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::ApiTimeout("x".into())),
            ErrorClass::NetworkTimeout
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::Network("x".into())),
            ErrorClass::NetworkTimeout
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::AudioCorrupt("x".into())),
            ErrorClass::AudioCorrupt
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::WorkerKilled("x".into())),
            ErrorClass::WorkerKilled
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::Cancelled),
            ErrorClass::Cancelled
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::Transcription(
                "model_not_selected: transcription model is not selected".into()
            )),
            ErrorClass::ModelNotSelected,
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::Transcription(
                "Модель не найдена или повреждена: ...".into()
            )),
            ErrorClass::ModelMissing,
        );
        assert_eq!(
            ErrorClass::from_core_error(&CoreError::Transcription("decode failed".into())),
            ErrorClass::Unknown,
        );
    }

    #[test]
    fn retryable_classes_are_transient_only() {
        // Transient — worth a backoff retry.
        for c in [
            ErrorClass::NetworkTimeout,
            ErrorClass::ApiQuota,
            ErrorClass::WorkerKilled,
            ErrorClass::Unknown,
        ] {
            assert!(c.is_retryable(), "{} should retry", c.as_str());
        }
        // Permanent — re-running fails identically, so surface immediately.
        for c in [
            ErrorClass::ApiAuth,
            ErrorClass::AudioCorrupt,
            ErrorClass::ModelNotSelected,
            ErrorClass::ModelMissing,
            ErrorClass::Cancelled,
        ] {
            assert!(!c.is_retryable(), "{} should fail fast", c.as_str());
        }
    }
}
