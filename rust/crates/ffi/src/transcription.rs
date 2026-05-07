use std::path::PathBuf;
use std::sync::Arc;
use meeting_core::usecases::{submit_transcription_job, get_job_status, transcribe_audio_file};
use crate::app_core::AppCore;
use crate::types::{AppError, FfiResult, JobDto, job_to_dto};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn transcribe_file(
        self: Arc<Self>,
        path: String,
    ) -> FfiResult<String> {
        transcribe_audio_file(Arc::clone(&self.transcriber), &PathBuf::from(path))
            .await
            .map(|t| t.text)
            .map_err(AppError::general)
    }

    pub async fn job_submit(
        self: Arc<Self>,
        path: String,
        name: Option<String>,
    ) -> FfiResult<JobDto> {
        let job_name = name.unwrap_or_else(|| "Untitled meeting".to_string());
        let job = submit_transcription_job(
            Arc::clone(&self.meeting_repo),
            Arc::clone(&self.job_repo),
            PathBuf::from(path),
            job_name,
        )
        .await
        .map_err(AppError::general)?;

        Ok(job_to_dto(job))
    }

    pub async fn job_status(
        self: Arc<Self>,
        id: String,
    ) -> FfiResult<Option<JobDto>> {
        let job = get_job_status(Arc::clone(&self.job_repo), &id)
            .await
            .map_err(AppError::general)?;

        Ok(job.map(job_to_dto))
    }
}
