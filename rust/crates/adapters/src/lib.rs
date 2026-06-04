mod whisper;
pub use whisper::{LazyWhisperTranscriber, TranscriberPrefs, WhisperTranscriber};

mod fs_meeting_file_store;
pub use fs_meeting_file_store::FsMeetingFileStore;

pub mod settings_store;
pub use settings_store::JsonSettingsStore;

pub mod transcription_models;
pub use transcription_models::{
    find_transcription_model, managed_model_path, resolve_transcription_model_path,
    transcription_model_catalog, ModelPathError, ModelPathErrorCode,
    TranscriptionModelCatalogEntry,
};

pub mod secret_store;
pub use secret_store::KeyringSecretStore;

pub mod db;
pub use db::{Db, SqliteJobRepo, SqliteMeetingRepo};

mod worker;
pub use worker::{LiveJobs, Worker, PROTOCOL_KINDS, TRANSCRIBE_KINDS};

pub mod llm;
pub use llm::{
    build_llm, probe_llm, AnthropicProvider, GeminiProvider, LlmConfig, OpenAiCompatProvider,
    ProviderKind, SwappableLlm,
};

pub mod templates;
pub use templates::FileTemplateLoader;

pub mod audio;
pub use audio::{run_recovery, CpalAudioCapture, RecoveryReport, WavRecovery};
