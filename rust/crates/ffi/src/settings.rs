use std::sync::Arc;
use meeting_adapters::settings_store::{PersistedPaths, PersistedTranscriberPrefs, RecordingPrefs};
use crate::app_core::{AppCore, xdg_data_dir};
use crate::types::{AppError, FfiResult, ModelInfoDto, RecordingPrefsDto, SettingsDto, SettingsPathsDto, TranscriberPrefsDto};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn settings_get(self: Arc<Self>) -> FfiResult<SettingsDto> {
        let s = self.settings.load();
        Ok(SettingsDto {
            paths: SettingsPathsDto {
                model:        s.paths.model.or_else(|| Some(self.model_path.display().to_string())),
                db:           s.paths.db.or_else(|| Some(self.db_path.display().to_string())),
                meetings_dir: s.paths.meetings_dir.or_else(|| Some(self.meetings_dir.display().to_string())),
                prompts:      s.paths.prompts.or_else(|| Some(self.prompts_dir.display().to_string())),
            },
            // Never expose the stored key back to the UI.
            anthropic_api_key: None,
            recording: RecordingPrefsDto {
                source:      s.recording.source,
                echo_cancel: s.recording.echo_cancel,
            },
            default_template: s.default_template,
            transcriber: TranscriberPrefsDto {
                language:  s.transcriber.language,
                beam_size: s.transcriber.beam_size,
                n_threads: s.transcriber.n_threads,
            },
        })
    }

    pub async fn settings_set(self: Arc<Self>, dto: SettingsDto) -> FfiResult<()> {
        let mut s = self.settings.load();

        let new_model_path = non_empty(dto.paths.model.clone());
        s.paths = PersistedPaths {
            model:        non_empty(dto.paths.model),
            db:           non_empty(dto.paths.db),
            meetings_dir: non_empty(dto.paths.meetings_dir),
            prompts:      non_empty(dto.paths.prompts),
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
        s.transcriber = PersistedTranscriberPrefs {
            language:  dto.transcriber.language.clone(),
            beam_size: dto.transcriber.beam_size,
            n_threads: dto.transcriber.n_threads,
        };

        // Apply new transcriber prefs to the live transcriber immediately.
        use std::path::PathBuf;
        use meeting_adapters::TranscriberPrefs;
        self.lazy_transcriber.set_prefs(TranscriberPrefs::new(
            dto.transcriber.language,
            dto.transcriber.beam_size,
            dto.transcriber.n_threads,
        ));

        // If model path changed, hot-swap — unloads old model immediately.
        if let Some(new_model) = new_model_path {
            let current = self.lazy_transcriber.current_model_path();
            if PathBuf::from(&new_model) != current {
                self.lazy_transcriber.set_model_path(PathBuf::from(new_model)).await;
            }
        }

        self.settings.save(s).map_err(AppError::general)
    }

    /// Returns all Whisper .bin models found in the default models directory
    /// (~/.local/share/meeting-assistant/models/) plus the directory of the
    /// currently configured model, sorted by file size descending.
    pub async fn models_list(self: Arc<Self>) -> FfiResult<Vec<ModelInfoDto>> {
        let models_dir = xdg_data_dir().join("meeting-assistant/models");

        let mut dirs_to_scan = vec![models_dir.clone()];
        if let Some(parent) = self.model_path.parent() {
            if parent != models_dir {
                dirs_to_scan.push(parent.to_path_buf());
            }
        }

        let mut seen = std::collections::HashSet::new();
        let mut models = Vec::new();

        for dir in dirs_to_scan {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("bin") { continue }
                if !seen.insert(path.clone()) { continue }

                let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let size_mb = std::fs::metadata(&path)
                    .map(|m| (m.len() / (1024 * 1024)) as u32)
                    .unwrap_or(0);
                let description = model_description(&filename).to_owned();

                models.push(ModelInfoDto { path: path.display().to_string(), name, description, size_mb });
            }
        }

        models.sort_by(|a, b| b.size_mb.cmp(&a.size_mb));
        Ok(models)
    }
}

fn model_description(filename: &str) -> &'static str {
    let f = &filename.to_lowercase();
    if f.contains("large-v3-turbo") && f.contains("q5_0") {
        "Рекомендуется · 809M пар., ≈large-v2 качество, быстрее large-v3 в 8x"
    } else if f.contains("large-v3-turbo") && f.contains("q8_0") {
        "809M пар., чуть точнее q5_0, немного медленнее"
    } else if f.contains("large-v3-turbo") {
        "809M пар., ≈large-v2 качество, быстрее large-v3 в 8x"
    } else if f.contains("large-v3") && f.contains("q5_0") {
        "Максимальное качество, квантизованная"
    } else if f.contains("large-v3") {
        "Максимальное качество, ~1.5 GB"
    } else if f.contains("large-v2") {
        "Высокое качество"
    } else if f.contains("large") {
        "Высокое качество"
    } else if f.contains("medium") && f.contains("q5_0") {
        "Среднее качество, квантизованная"
    } else if f.contains("medium") {
        "Среднее качество, ~1.5 GB"
    } else if f.contains("small") {
        "Быстрая, меньше RAM"
    } else if f.contains("base") {
        "Базовое качество, очень быстрая"
    } else if f.contains("tiny") {
        "Минимальная, ~75 MB"
    } else {
        "Whisper модель"
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty())
}
