use std::sync::Arc;
use meeting_core::ports::CaptureSource;
use meeting_core::usecases::{start_recording, stop_recording};
use crate::app_core::AppCore;
use crate::types::{AppError, FfiResult, RecordingDto};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn recording_start(
        self: Arc<Self>,
        name: Option<String>,
        source: Option<String>,
        echo_cancel: Option<bool>,
    ) -> FfiResult<RecordingDto> {
        let capture_source = match source.as_deref() {
            Some("system") => CaptureSource::System,
            Some("mixed")  => CaptureSource::Mixed,
            _              => CaptureSource::Mic,
        };
        let meeting = start_recording(
            Arc::clone(&self.audio_capture),
            Arc::clone(&self.meeting_repo),
            &self.recordings_dir,
            name,
            capture_source,
            echo_cancel.unwrap_or(false),
        )
        .await
        .map_err(AppError::general)?;

        Ok(RecordingDto {
            id:         meeting.id,
            name:       meeting.name,
            audio_path: meeting.audio_path.display().to_string(),
        })
    }

    pub async fn recording_stop(
        self: Arc<Self>,
        id: String,
    ) -> FfiResult<RecordingDto> {
        let meeting = stop_recording(
            Arc::clone(&self.audio_capture),
            Arc::clone(&self.meeting_repo),
            &id,
        )
        .await
        .map_err(AppError::general)?;

        Ok(RecordingDto {
            id:         meeting.id,
            name:       meeting.name,
            audio_path: meeting.audio_path.display().to_string(),
        })
    }
}
