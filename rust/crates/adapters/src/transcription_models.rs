use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings_store::{PersistedSettings, TranscriptionModelSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TranscriptionModelCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub approximate_size: &'static str,
    pub description: &'static str,
    pub pros: &'static [&'static str],
    pub cons: &'static [&'static str],
    pub filename: &'static str,
    pub download_url: &'static str,
    pub checksum_sha1: &'static str,
}

pub const TRANSCRIPTION_MODEL_CATALOG: &[TranscriptionModelCatalogEntry] = &[
    TranscriptionModelCatalogEntry {
        id: "tiny",
        display_name: "Tiny",
        approximate_size: "75 MiB",
        description: "Fastest multilingual model for quick drafts and short notes.",
        pros: &["Very fast", "Small download"],
        cons: &["Lowest accuracy", "Struggles with noisy meetings"],
        filename: "ggml-tiny.bin",
        download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        checksum_sha1: "bd577a113a864445d4c299885e0cb97d4ba92b5f",
    },
    TranscriptionModelCatalogEntry {
        id: "base",
        display_name: "Base",
        approximate_size: "142 MiB",
        description: "Lightweight multilingual model with better accuracy than tiny.",
        pros: &["Fast on most machines", "Good for clear speech"],
        cons: &["Limited accuracy on complex meetings"],
        filename: "ggml-base.bin",
        download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        checksum_sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
    },
    TranscriptionModelCatalogEntry {
        id: "small",
        display_name: "Small",
        approximate_size: "466 MiB",
        description: "Balanced multilingual model for everyday meeting transcription.",
        pros: &["Noticeably better accuracy", "Moderate size"],
        cons: &["Slower than tiny/base"],
        filename: "ggml-small.bin",
        download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        checksum_sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
    },
    TranscriptionModelCatalogEntry {
        id: "medium",
        display_name: "Medium",
        approximate_size: "1.5 GiB",
        description: "Higher-quality multilingual model for harder audio and longer meetings.",
        pros: &["Strong accuracy", "Better with accents and noise"],
        cons: &["Large download", "Needs more CPU and memory"],
        filename: "ggml-medium.bin",
        download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        checksum_sha1: "fd9727b6e1217c2f614f9b698455c4ffd82463b4",
    },
    TranscriptionModelCatalogEntry {
        id: "large-v3",
        display_name: "Large v3",
        approximate_size: "2.9 GiB",
        description: "Highest-quality multilingual model in the built-in catalog.",
        pros: &["Best quality", "Most robust on difficult audio"],
        cons: &["Very large download", "Slowest and most memory hungry"],
        filename: "ggml-large-v3.bin",
        download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        checksum_sha1: "ad82bf6a9043ceed055076d0fd39f5f186ff8062",
    },
    TranscriptionModelCatalogEntry {
        id: "large-v3-turbo-q5_0",
        display_name: "Large v3 Turbo Q5",
        approximate_size: "548 MiB",
        description: "Compressed Turbo model for faster everyday transcription with moderate size.",
        pros: &["Fast for a large model", "Moderate download size"],
        cons: &[
            "Quantized quality trade-off",
            "Less accurate than full Large v3",
        ],
        filename: "ggml-large-v3-turbo-q5_0.bin",
        download_url:
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        checksum_sha1: "e050f7970618a659205450ad97eb95a18d69c9ee",
    },
    TranscriptionModelCatalogEntry {
        id: "large-v3-turbo-q8_0",
        display_name: "Large v3 Turbo Q8",
        approximate_size: "834 MiB",
        description: "Higher-precision Turbo model with stronger quality than Q5 at a larger size.",
        pros: &["Fast for a large model", "Better quality than Q5"],
        cons: &["Larger download", "Still a quantized model"],
        filename: "ggml-large-v3-turbo-q8_0.bin",
        download_url:
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin",
        checksum_sha1: "01bf15bedffe9f39d65c1b6ff9b687ea91f59e0e",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPathErrorCode {
    ModelNotSelected,
    ModelMissing,
}

impl ModelPathErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelNotSelected => "model_not_selected",
            Self::ModelMissing => "model_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPathError {
    pub code: ModelPathErrorCode,
    pub path: Option<PathBuf>,
    pub message: String,
}

impl ModelPathError {
    fn not_selected() -> Self {
        Self {
            code: ModelPathErrorCode::ModelNotSelected,
            path: None,
            message: "transcription model is not selected".to_string(),
        }
    }

    fn missing(path: impl Into<Option<PathBuf>>, message: impl Into<String>) -> Self {
        Self {
            code: ModelPathErrorCode::ModelMissing,
            path: path.into(),
            message: message.into(),
        }
    }
}

pub fn transcription_model_catalog() -> &'static [TranscriptionModelCatalogEntry] {
    TRANSCRIPTION_MODEL_CATALOG
}

pub fn find_transcription_model(id: &str) -> Option<&'static TranscriptionModelCatalogEntry> {
    TRANSCRIPTION_MODEL_CATALOG
        .iter()
        .find(|entry| entry.id == id)
}

pub fn resolve_transcription_model_path(
    settings: &PersistedSettings,
) -> Result<PathBuf, ModelPathError> {
    match settings.transcriber.model_source {
        TranscriptionModelSource::Managed => resolve_managed_path(settings),
        TranscriptionModelSource::CustomPath => resolve_custom_path(settings),
    }
}

pub fn managed_model_path(models_dir: &Path, model_id: &str) -> Option<PathBuf> {
    find_transcription_model(model_id).map(|entry| models_dir.join(entry.filename))
}

fn resolve_managed_path(settings: &PersistedSettings) -> Result<PathBuf, ModelPathError> {
    let Some(model_id) = settings.transcriber.model_id.as_deref() else {
        return Err(ModelPathError::not_selected());
    };
    let Some(path) = managed_model_path(&settings.effective_models_dir(), model_id) else {
        return Err(ModelPathError::missing(
            None,
            format!("unknown transcription model id: {model_id}"),
        ));
    };
    require_model_file(path)
}

fn resolve_custom_path(settings: &PersistedSettings) -> Result<PathBuf, ModelPathError> {
    let Some(path) = settings
        .transcriber
        .custom_model_path
        .as_deref()
        .map(PathBuf::from)
    else {
        return Err(ModelPathError::not_selected());
    };
    require_model_file(path)
}

fn require_model_file(path: PathBuf) -> Result<PathBuf, ModelPathError> {
    if is_valid_model_file(&path) {
        Ok(path)
    } else {
        Err(ModelPathError::missing(
            Some(path.clone()),
            format!(
                "transcription model is missing or invalid: {}",
                path.display()
            ),
        ))
    }
}

fn is_valid_model_file(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("ggml-") && name.ends_with(".bin"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_store::{PersistedPaths, PersistedTranscriberPrefs};
    use std::fs;

    #[test]
    fn catalog_contains_builtin_models() {
        let ids: Vec<_> = transcription_model_catalog()
            .iter()
            .map(|entry| entry.id)
            .collect();

        assert_eq!(
            ids,
            vec![
                "tiny",
                "base",
                "small",
                "medium",
                "large-v3",
                "large-v3-turbo-q5_0",
                "large-v3-turbo-q8_0"
            ]
        );
    }

    #[test]
    fn managed_path_returns_model_not_selected_without_choice() {
        let settings = PersistedSettings::default();

        let err = resolve_transcription_model_path(&settings).unwrap_err();

        assert_eq!(err.code.as_str(), "model_not_selected");
    }

    #[test]
    fn managed_path_resolves_installed_catalog_model() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_path = dir.path().join("ggml-base.bin");
        fs::write(&model_path, b"not a real model, enough for path validation")
            .expect("write model");
        let mut settings = PersistedSettings::default();
        settings.paths.models_dir = Some(dir.path().display().to_string());
        settings.transcriber.model_id = Some("base".to_string());

        let resolved = resolve_transcription_model_path(&settings).expect("model should resolve");

        assert_eq!(resolved, model_path);
    }

    #[test]
    fn managed_path_returns_model_missing_when_file_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = PersistedSettings::default();
        settings.paths.models_dir = Some(dir.path().display().to_string());
        settings.transcriber.model_id = Some("small".to_string());

        let err = resolve_transcription_model_path(&settings).unwrap_err();

        assert_eq!(err.code.as_str(), "model_missing");
        assert_eq!(err.path, Some(dir.path().join("ggml-small.bin")));
    }

    #[test]
    fn custom_path_resolves_existing_external_model() {
        let dir = tempfile::tempdir().expect("tempdir");
        let model_path = dir.path().join("ggml-custom.bin");
        fs::write(&model_path, b"not a real model, enough for path validation")
            .expect("write model");
        let mut settings = PersistedSettings::default();
        settings.transcriber = PersistedTranscriberPrefs {
            model_source: TranscriptionModelSource::CustomPath,
            custom_model_path: Some(model_path.display().to_string()),
            ..PersistedTranscriberPrefs::default()
        };

        let resolved = resolve_transcription_model_path(&settings).expect("model should resolve");

        assert_eq!(resolved, model_path);
    }

    #[test]
    fn effective_models_dir_uses_default_app_data_models_directory() {
        let settings = PersistedSettings {
            paths: PersistedPaths::default(),
            ..PersistedSettings::default()
        };

        assert!(settings
            .effective_models_dir()
            .ends_with("meeting-assistant/models"));
    }
}
