use std::path::PathBuf;
use std::sync::Arc;

use meeting_adapters::settings_store::{PersistedSettings, TranscriptionModelSource};
use meeting_adapters::{
    resolve_transcription_model_path, JsonSettingsStore, KeyringSecretStore, ModelPathError,
    SecretStatus, VaultState,
};
use meeting_api::{
    InstallationView, MinutesSetup, MinutesSetupState, RecoveredRecording, SetupStatus,
    SetupStatusService, TranscriptionSetup, TranscriptionSetupState,
};
use meeting_core::ports::MeetingRepo;

use crate::transcription_model_service::AppTranscriptionModelService;

/// Concrete [`SetupStatusService`] over the settings store, the secret store
/// and the model installer. It reads files and in-memory state only.
pub struct AppSetupStatusService {
    settings_store: Arc<JsonSettingsStore>,
    secrets: Arc<KeyringSecretStore>,
    models: AppTranscriptionModelService,
    recovered: Vec<RecoveredRecording>,
}

impl AppSetupStatusService {
    pub fn new(
        settings_store: Arc<JsonSettingsStore>,
        secrets: Arc<KeyringSecretStore>,
        models: AppTranscriptionModelService,
        recovered: Vec<RecoveredRecording>,
    ) -> Self {
        Self {
            settings_store,
            secrets,
            models,
            recovered,
        }
    }
}

impl SetupStatusService for AppSetupStatusService {
    fn status(&self) -> SetupStatus {
        let settings = self.settings_store.load().normalize();
        let provider = settings.llm.active;
        let locked = matches!(
            self.secrets.status(),
            SecretStatus::Vault {
                state: VaultState::Locked,
                ..
            }
        );
        SetupStatus {
            transcription: transcription_setup(
                &settings,
                resolve_transcription_model_path(&settings),
                self.models.active_installation(),
            ),
            minutes: MinutesSetup {
                state: minutes_state(
                    provider.needs_key(),
                    locked,
                    // `has_stored`, not `has_key`: no keyring read at launch.
                    || self.secrets.has_stored(provider.as_str()),
                    settings.minutes_setup_skipped,
                ),
                provider: provider.as_str().to_string(),
            },
            cpu_supported: cpu_meets_baseline(),
            recovered_recordings: self.recovered.clone(),
        }
    }
}

fn transcription_setup(
    settings: &PersistedSettings,
    resolved: Result<PathBuf, ModelPathError>,
    installation: Option<InstallationView>,
) -> TranscriptionSetup {
    let (state, reason) = match (&resolved, &installation) {
        (Ok(_), _) => (TranscriptionSetupState::Ready, None),
        (Err(e), Some(_)) => (TranscriptionSetupState::Downloading, Some(e.code)),
        (Err(e), None) => (TranscriptionSetupState::Missing, Some(e.code)),
    };
    let managed = settings.transcriber.model_source == TranscriptionModelSource::Managed;
    TranscriptionSetup {
        state,
        reason: reason.map(|code| code.as_str().to_string()),
        source: if managed { "managed" } else { "custom_path" }.to_string(),
        model_id: settings.transcriber.model_id.clone().filter(|_| managed),
        recommended_model_id: recommended_model_id().to_string(),
        installation,
    }
}

/// `has_stored` is a closure so that a keyless provider or a locked vault never
/// reaches the secret store at all.
fn minutes_state(
    needs_key: bool,
    locked: bool,
    has_stored: impl FnOnce() -> bool,
    skipped: bool,
) -> MinutesSetupState {
    if !needs_key {
        MinutesSetupState::Ready
    } else if locked {
        MinutesSetupState::Locked
    } else if has_stored() {
        MinutesSetupState::Ready
    } else if skipped {
        MinutesSetupState::Skipped
    } else {
        MinutesSetupState::Missing
    }
}

/// The model offered to a user who has none. Not measured yet: Turbo Q5 where
/// Metal carries it, Small for the CPU-only Windows and Linux builds. Confirm
/// with `rust/scripts/bench_transcribe.sh` before 1.0.
fn recommended_model_id() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "large-v3-turbo-q5_0"
    } else {
        "small"
    }
}

/// Whether this CPU has what the packaged Whisper build is compiled for (see
/// "CPU baseline" in `packaging/README.md`). Below it the sidecar dies with an
/// illegal instruction the first time it transcribes.
fn cpu_meets_baseline() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
            && std::arch::is_x86_feature_detected!("f16c")
            && std::arch::is_x86_feature_detected!("bmi2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        true
    }
}

/// The meetings behind the WAVs the recovery pass repaired, for the "we
/// recovered your recording" note. A WAV with no row is skipped: there is
/// nothing to link to.
pub async fn recovered_recordings(
    repaired: &[PathBuf],
    repo: &dyn MeetingRepo,
) -> Vec<RecoveredRecording> {
    let mut out = Vec::new();
    for audio_path in repaired {
        match repo.find_by_audio_path(audio_path).await {
            Ok(Some(meeting)) => out.push(RecoveredRecording {
                meeting_id: meeting.id,
                name: meeting.name,
            }),
            Ok(None) => {}
            Err(e) => tracing::warn!(
                "setup status: cannot look up recovered {}: {e}",
                audio_path.display()
            ),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_adapters::find_transcription_model;
    use meeting_adapters::settings_store::PersistedTranscriberPrefs;

    fn settings_with_model(models_dir: &std::path::Path, model_id: &str) -> PersistedSettings {
        let mut settings = PersistedSettings {
            transcriber: PersistedTranscriberPrefs {
                model_source: TranscriptionModelSource::Managed,
                model_id: Some(model_id.to_string()),
                ..PersistedTranscriberPrefs::default()
            },
            ..PersistedSettings::default()
        };
        settings.paths.models_dir = Some(models_dir.display().to_string());
        settings
    }

    fn setup(settings: &PersistedSettings, installing: bool) -> TranscriptionSetup {
        let installation = installing.then(|| InstallationView {
            job_id: "job".to_string(),
            model_id: "small".to_string(),
            status: "downloading".to_string(),
            bytes_downloaded: 1,
            total_bytes: Some(4),
            percent: Some(25.0),
            error: None,
        });
        transcription_setup(
            settings,
            resolve_transcription_model_path(settings),
            installation,
        )
    }

    #[test]
    fn new_profile_has_no_model_selected() {
        let t = setup(&PersistedSettings::default(), false);
        assert_eq!(t.state, TranscriptionSetupState::Missing);
        assert_eq!(t.reason.as_deref(), Some("model_not_selected"));
        assert_eq!(t.source, "managed");
        assert_eq!(t.model_id, None);
    }

    #[test]
    fn selected_model_with_its_file_is_ready() {
        let models = tempfile::tempdir().unwrap();
        std::fs::write(models.path().join("ggml-base.bin"), b"model").unwrap();
        let t = setup(&settings_with_model(models.path(), "base"), false);
        assert_eq!(t.state, TranscriptionSetupState::Ready);
        assert_eq!(t.reason, None);
        assert_eq!(t.model_id.as_deref(), Some("base"));
    }

    #[test]
    fn selected_model_without_its_file_is_missing_not_unselected() {
        let models = tempfile::tempdir().unwrap();
        let t = setup(&settings_with_model(models.path(), "base"), false);
        assert_eq!(t.state, TranscriptionSetupState::Missing);
        assert_eq!(t.reason.as_deref(), Some("model_missing"));
    }

    #[test]
    fn a_running_download_shows_until_a_model_is_usable() {
        let t = setup(&PersistedSettings::default(), true);
        assert_eq!(t.state, TranscriptionSetupState::Downloading);
        assert_eq!(t.installation.unwrap().percent, Some(25.0));
    }

    #[test]
    fn a_second_download_does_not_hide_a_ready_model() {
        let models = tempfile::tempdir().unwrap();
        std::fs::write(models.path().join("ggml-base.bin"), b"model").unwrap();
        let t = setup(&settings_with_model(models.path(), "base"), true);
        assert_eq!(t.state, TranscriptionSetupState::Ready);
        assert!(t.installation.is_some());
    }

    #[test]
    fn custom_model_file_counts_as_ready() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ggml-own.bin");
        std::fs::write(&path, b"model").unwrap();
        let settings = PersistedSettings {
            transcriber: PersistedTranscriberPrefs {
                model_source: TranscriptionModelSource::CustomPath,
                model_id: Some("base".to_string()),
                custom_model_path: Some(path.display().to_string()),
                ..PersistedTranscriberPrefs::default()
            },
            ..PersistedSettings::default()
        };
        let t = setup(&settings, false);
        assert_eq!(t.state, TranscriptionSetupState::Ready);
        assert_eq!(t.source, "custom_path");
        assert_eq!(t.model_id, None, "a custom file has no catalogue id");
    }

    #[test]
    fn recommended_model_is_in_the_catalogue() {
        assert!(find_transcription_model(recommended_model_id()).is_some());
    }

    #[test]
    fn minutes_states() {
        use MinutesSetupState::*;
        let never = || -> bool { panic!("the secret store must not be asked") };
        assert_eq!(minutes_state(false, false, never, false), Ready, "keyless");
        assert_eq!(minutes_state(true, true, never, true), Locked);
        assert_eq!(minutes_state(true, false, || true, false), Ready);
        assert_eq!(minutes_state(true, false, || true, true), Ready);
        assert_eq!(minutes_state(true, false, || false, true), Skipped);
        assert_eq!(minutes_state(true, false, || false, false), Missing);
    }
}
