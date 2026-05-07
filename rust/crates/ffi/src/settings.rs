use std::sync::Arc;
use meeting_adapters::settings_store::{PersistedPaths, RecordingPrefs};
use crate::app_core::AppCore;
use crate::types::{AppError, FfiResult, RecordingPrefsDto, SettingsDto, SettingsPathsDto};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn settings_get(self: Arc<Self>) -> FfiResult<SettingsDto> {
        let s = self.settings.load();
        Ok(SettingsDto {
            paths: SettingsPathsDto {
                model:      s.paths.model.or_else(|| Some(self.model_path.display().to_string())),
                db:         s.paths.db.or_else(|| Some(self.db_path.display().to_string())),
                recordings: s.paths.recordings.or_else(|| Some(self.recordings_dir.display().to_string())),
                prompts:    s.paths.prompts.or_else(|| Some(self.prompts_dir.display().to_string())),
            },
            // Never expose the stored key back to the UI.
            anthropic_api_key: None,
            recording: RecordingPrefsDto {
                source:      s.recording.source,
                echo_cancel: s.recording.echo_cancel,
            },
            default_template: s.default_template,
        })
    }

    pub async fn settings_set(self: Arc<Self>, dto: SettingsDto) -> FfiResult<()> {
        let mut s = self.settings.load();

        s.paths = PersistedPaths {
            model:      non_empty(dto.paths.model),
            db:         non_empty(dto.paths.db),
            recordings: non_empty(dto.paths.recordings),
            prompts:    non_empty(dto.paths.prompts),
        };

        // None = "don't touch". Some("") = "clear". Some(key) = "update".
        if let Some(key) = dto.anthropic_api_key {
            if key.trim().is_empty() {
                s.anthropic_api_key = None;
            } else {
                // Apply to env for the current process immediately.
                unsafe { std::env::set_var("ANTHROPIC_API_KEY", &key); }
                s.anthropic_api_key = Some(key);
            }
        }

        s.recording = RecordingPrefs {
            source:      dto.recording.source,
            echo_cancel: dto.recording.echo_cancel,
        };
        s.default_template = non_empty(dto.default_template);

        self.settings.save(s).map_err(AppError::general)
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty())
}
