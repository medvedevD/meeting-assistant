use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use meeting_adapters::settings_store::TranscriptionModelSource;
use meeting_adapters::{find_transcription_model, transcription_model_catalog, JsonSettingsStore};
use meeting_api::{
    InstallStarted, InstallationView, ModelServiceError, TranscriptionModelService,
    TranscriptionModelView, TranscriptionModelsView,
};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
enum InstallStatus {
    Queued,
    Downloading,
    Done,
    Failed,
}

impl InstallStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
struct InstallJob {
    job_id: String,
    model_id: String,
    status: InstallStatus,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
}

impl InstallJob {
    fn view(&self) -> InstallationView {
        let percent = self.total_bytes.and_then(|total| {
            (total > 0).then(|| (self.bytes_downloaded as f64 / total as f64 * 100.0).min(100.0))
        });
        InstallationView {
            job_id: self.job_id.clone(),
            model_id: self.model_id.clone(),
            status: self.status.as_str().to_string(),
            bytes_downloaded: self.bytes_downloaded,
            total_bytes: self.total_bytes,
            percent,
            error: self.error.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AppTranscriptionModelService {
    settings_store: Arc<JsonSettingsStore>,
    jobs: Arc<DashMap<String, InstallJob>>,
    client: reqwest::Client,
}

impl AppTranscriptionModelService {
    pub fn new(settings_store: Arc<JsonSettingsStore>) -> Self {
        cleanup_partial_downloads(&settings_store.load().normalize().effective_models_dir());
        Self {
            settings_store,
            jobs: Arc::new(DashMap::new()),
            client: reqwest::Client::new(),
        }
    }

    fn settings(&self) -> meeting_adapters::settings_store::PersistedSettings {
        self.settings_store.load().normalize()
    }

    fn models_dir(&self) -> PathBuf {
        self.settings().effective_models_dir()
    }

    fn job(&self, job_id: &str) -> Result<InstallJob, ModelServiceError> {
        self.jobs
            .get(job_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| {
                ModelServiceError::NotFound(format!("installation job not found: {job_id}"))
            })
    }

    fn set_job(&self, job: InstallJob) {
        self.jobs.insert(job.job_id.clone(), job);
    }

    fn new_job_id() -> Result<String, ModelServiceError> {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| ModelServiceError::Internal(format!("failed to create job id: {e}")))?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }

    async fn install_job(self, job_id: String, model_id: String) {
        let result = self.install_model(&job_id, &model_id).await;
        if let Err(error) = result {
            if let Ok(mut job) = self.job(&job_id) {
                job.status = InstallStatus::Failed;
                job.error = Some(error);
                self.set_job(job);
            }
        }
    }

    async fn install_model(&self, job_id: &str, model_id: &str) -> Result<(), String> {
        let entry = find_transcription_model(model_id)
            .ok_or_else(|| format!("unknown transcription model id: {model_id}"))?;
        let models_dir = self.models_dir();
        tokio::fs::create_dir_all(&models_dir)
            .await
            .map_err(|e| format!("failed to create models dir: {e}"))?;
        let final_path = models_dir.join(entry.filename);
        let tmp_path = models_dir.join(format!(".{}.{}.tmp", entry.filename, job_id));

        let attempts = 3;
        for attempt in 1..=attempts {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            match self
                .download_once(job_id, entry.download_url, &tmp_path)
                .await
            {
                Ok(()) => break,
                Err(error) if attempt < attempts && is_transient_download_error(&error) => {
                    tracing::warn!(
                        job_id,
                        model_id,
                        attempt,
                        "transient model download failure: {error}"
                    );
                    continue;
                }
                Err(error) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(error);
                }
            }
        }

        let actual = sha1_file(&tmp_path)
            .await
            .map_err(|e| format!("failed to checksum downloaded model: {e}"))?;
        if actual != entry.checksum_sha1 {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(format!(
                "checksum mismatch for {model_id}: expected {}, got {actual}",
                entry.checksum_sha1
            ));
        }

        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .map_err(|e| format!("failed to install model atomically: {e}"))?;

        let mut job = self.job(job_id).map_err(|e| e.message().to_string())?;
        job.status = InstallStatus::Done;
        job.bytes_downloaded = job.total_bytes.unwrap_or(job.bytes_downloaded);
        job.error = None;
        self.set_job(job);
        Ok(())
    }

    async fn download_once(&self, job_id: &str, url: &str, tmp_path: &Path) -> Result<(), String> {
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("download request failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("download returned HTTP {}", response.status()));
        }

        let total = response.content_length();
        let mut job = self.job(job_id).map_err(|e| e.message().to_string())?;
        job.status = InstallStatus::Downloading;
        job.total_bytes = total;
        job.bytes_downloaded = 0;
        job.error = None;
        self.set_job(job);

        let mut file = tokio::fs::File::create(tmp_path)
            .await
            .map_err(|e| format!("failed to create temp model file: {e}"))?;
        let mut downloaded = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("download stream failed: {e}"))?
        {
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("failed to write temp model file: {e}"))?;
            downloaded += chunk.len() as u64;
            let mut job = self.job(job_id).map_err(|e| e.message().to_string())?;
            job.bytes_downloaded = downloaded;
            job.total_bytes = total;
            self.set_job(job);
        }
        file.flush()
            .await
            .map_err(|e| format!("failed to flush temp model file: {e}"))?;
        Ok(())
    }
}

#[async_trait]
impl TranscriptionModelService for AppTranscriptionModelService {
    async fn list(&self) -> Result<TranscriptionModelsView, ModelServiceError> {
        let settings = self.settings();
        let models_dir = settings.effective_models_dir();
        let active_source = match settings.transcriber.model_source {
            TranscriptionModelSource::Managed => "managed",
            TranscriptionModelSource::CustomPath => "custom_path",
        };
        let selected_model_id = settings.transcriber.model_id.clone();
        let models = transcription_model_catalog()
            .iter()
            .map(|entry| {
                let path = models_dir.join(entry.filename);
                let active = settings.transcriber.model_source == TranscriptionModelSource::Managed
                    && selected_model_id.as_deref() == Some(entry.id);
                TranscriptionModelView {
                    id: entry.id.to_string(),
                    display_name: entry.display_name.to_string(),
                    approximate_size: entry.approximate_size.to_string(),
                    description: entry.description.to_string(),
                    pros: entry
                        .pros
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    cons: entry
                        .cons
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    filename: entry.filename.to_string(),
                    download_url: entry.download_url.to_string(),
                    checksum: entry.checksum_sha1.to_string(),
                    installed: path.is_file(),
                    active,
                }
            })
            .collect();

        Ok(TranscriptionModelsView {
            models,
            selected_model_id,
            active_source: active_source.to_string(),
            models_dir: models_dir.display().to_string(),
        })
    }

    async fn start_install(&self, model_id: String) -> Result<InstallStarted, ModelServiceError> {
        let entry = find_transcription_model(&model_id)
            .ok_or_else(|| ModelServiceError::NotFound(format!("unknown model: {model_id}")))?;
        let final_path = self.models_dir().join(entry.filename);
        if final_path.is_file() {
            return Err(ModelServiceError::Conflict(format!(
                "model is already installed: {model_id}"
            )));
        }
        if let Some(existing) = self.jobs.iter().find(|job| {
            job.model_id == model_id
                && matches!(
                    job.status,
                    InstallStatus::Queued | InstallStatus::Downloading
                )
        }) {
            return Ok(InstallStarted {
                job_id: existing.job_id.clone(),
            });
        }

        let job_id = Self::new_job_id()?;
        self.set_job(InstallJob {
            job_id: job_id.clone(),
            model_id: model_id.clone(),
            status: InstallStatus::Queued,
            bytes_downloaded: 0,
            total_bytes: None,
            error: None,
        });
        let worker = self.clone();
        let spawned_job_id = job_id.clone();
        tokio::spawn(async move {
            worker.install_job(spawned_job_id, model_id).await;
        });

        Ok(InstallStarted { job_id })
    }

    async fn installation(&self, job_id: String) -> Result<InstallationView, ModelServiceError> {
        self.job(&job_id).map(|job| job.view())
    }

    async fn delete_model(&self, model_id: String) -> Result<(), ModelServiceError> {
        let entry = find_transcription_model(&model_id)
            .ok_or_else(|| ModelServiceError::NotFound(format!("unknown model: {model_id}")))?;
        let settings = self.settings();
        if settings.transcriber.model_source == TranscriptionModelSource::Managed
            && settings.transcriber.model_id.as_deref() == Some(entry.id)
        {
            return Err(ModelServiceError::Conflict(
                "cannot delete the active transcription model".to_string(),
            ));
        }
        let path = settings.effective_models_dir().join(entry.filename);
        if !path.is_file() {
            return Err(ModelServiceError::NotFound(format!(
                "installed model not found: {model_id}"
            )));
        }
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| ModelServiceError::Internal(format!("failed to delete model: {e}")))
    }
}

fn is_transient_download_error(error: &str) -> bool {
    error.contains("request")
        || error.contains("stream")
        || error.contains("HTTP 408")
        || error.contains("HTTP 429")
        || error.contains("HTTP 5")
}

async fn sha1_file(path: &Path) -> std::io::Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn cleanup_partial_downloads(models_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(models_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_tmp = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".ggml-") && name.ends_with(".tmp"));
        if is_tmp {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_adapters::settings_store::{PersistedSettings, PersistedTranscriberPrefs};
    use meeting_api::TranscriptionModelService;

    fn store_with_settings(
        settings: PersistedSettings,
    ) -> (Arc<JsonSettingsStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(JsonSettingsStore::open(dir.path().join("settings.json")));
        store.save(settings).expect("save settings");
        (store, dir)
    }

    #[tokio::test]
    async fn delete_rejects_active_managed_model() {
        let models = tempfile::tempdir().expect("models dir");
        std::fs::write(models.path().join("ggml-base.bin"), b"installed").expect("write model");
        let mut settings = PersistedSettings::default();
        settings.paths.models_dir = Some(models.path().display().to_string());
        settings.transcriber.model_source = TranscriptionModelSource::Managed;
        settings.transcriber.model_id = Some("base".to_string());
        let (store, _settings_dir) = store_with_settings(settings);
        let service = AppTranscriptionModelService::new(store);

        let err = service.delete_model("base".to_string()).await.unwrap_err();

        assert!(matches!(err, ModelServiceError::Conflict(_)));
        assert!(models.path().join("ggml-base.bin").is_file());
    }

    #[tokio::test]
    async fn delete_removes_non_active_managed_model() {
        let models = tempfile::tempdir().expect("models dir");
        let model_path = models.path().join("ggml-base.bin");
        std::fs::write(&model_path, b"installed").expect("write model");
        let mut settings = PersistedSettings::default();
        settings.paths.models_dir = Some(models.path().display().to_string());
        settings.transcriber = PersistedTranscriberPrefs {
            model_source: TranscriptionModelSource::Managed,
            model_id: Some("small".to_string()),
            ..PersistedTranscriberPrefs::default()
        };
        let (store, _settings_dir) = store_with_settings(settings);
        let service = AppTranscriptionModelService::new(store);

        service.delete_model("base".to_string()).await.unwrap();

        assert!(!model_path.exists());
    }

    #[test]
    fn new_cleans_partial_temp_downloads() {
        let models = tempfile::tempdir().expect("models dir");
        let tmp = models.path().join(".ggml-base.bin.job.tmp");
        let keep = models.path().join("ggml-base.bin");
        std::fs::write(&tmp, b"partial").expect("write tmp");
        std::fs::write(&keep, b"installed").expect("write installed");
        let mut settings = PersistedSettings::default();
        settings.paths.models_dir = Some(models.path().display().to_string());
        let (store, _settings_dir) = store_with_settings(settings);

        let _service = AppTranscriptionModelService::new(store);

        assert!(!tmp.exists());
        assert!(keep.exists());
    }
}
