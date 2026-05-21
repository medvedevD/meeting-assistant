mod whisper;
pub use whisper::{LazyWhisperTranscriber, WhisperTranscriber, TranscriberPrefs};

mod fs_meeting_file_store;
pub use fs_meeting_file_store::FsMeetingFileStore;

pub mod settings_store;
pub use settings_store::JsonSettingsStore;

pub mod secret_store;
pub use secret_store::KeyringSecretStore;

pub mod db;
pub use db::{Db, SqliteMeetingRepo, SqliteJobRepo};

mod worker;
pub use worker::{LiveProgress, Worker};

pub mod llm;
pub use llm::{
    build_llm, probe_llm, AnthropicProvider, GeminiProvider, LlmConfig, OpenAiCompatProvider,
    ProviderKind, SwappableLlm,
};

pub mod templates;
pub use templates::FileTemplateLoader;

pub mod audio;
pub use audio::{run_recovery, CpalAudioCapture, RecoveryReport, WavRecovery};
