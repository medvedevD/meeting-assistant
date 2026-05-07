use std::sync::Arc;
use meeting_core::usecases::{generate_protocol, list_meetings};
use crate::app_core::AppCore;
use crate::types::{AppError, FfiResult, MeetingDto, ProtocolDto};

#[uniffi::export(async_runtime = "tokio")]
impl AppCore {
    pub async fn meetings_list(self: Arc<Self>) -> FfiResult<Vec<MeetingDto>> {
        let meetings = list_meetings(Arc::clone(&self.meeting_repo))
            .await
            .map_err(AppError::general)?;

        Ok(meetings
            .into_iter()
            .map(|m| MeetingDto {
                has_transcript: m.transcript_text.is_some(),
                has_protocol:   m.protocol_text.is_some(),
                id:         m.id,
                name:       m.name,
                audio_path: m.audio_path.display().to_string(),
                created_at: m.created_at,
            })
            .collect())
    }

    pub async fn protocol_load(
        self: Arc<Self>,
        meeting_id: String,
    ) -> FfiResult<Option<ProtocolDto>> {
        let meeting = self.meeting_repo
            .find_by_id(&meeting_id).await
            .map_err(AppError::general)?;

        Ok(meeting.and_then(|m| m.protocol_text.map(|md| ProtocolDto { markdown: md })))
    }

    pub async fn protocol_generate(
        self: Arc<Self>,
        meeting_id: String,
        template_name: Option<String>,
    ) -> FfiResult<ProtocolDto> {
        let meeting = self.meeting_repo
            .find_by_id(&meeting_id).await
            .map_err(AppError::general)?
            .ok_or_else(|| AppError::general(format!("meeting '{meeting_id}' not found")))?;

        let transcript = meeting.transcript_text
            .ok_or_else(|| AppError::general("meeting has no transcript yet"))?;

        let protocol = generate_protocol(
            Arc::clone(&self.llm),
            Arc::clone(&self.templates),
            &transcript,
            template_name.as_deref(),
            Some(&meeting.name),
        )
        .await
        .map_err(AppError::general)?;

        self.meeting_repo
            .save_protocol(&meeting_id, &protocol.markdown)
            .await
            .map_err(AppError::general)?;

        Ok(ProtocolDto { markdown: protocol.markdown })
    }
}
