package repository

import domain.JobProgress
import domain.JobStatus
import domain.Meeting
import domain.Protocol
import uniffi.meeting_assistant_ffi.AppCore

class UniffiMeetingRepository(private val core: AppCore) : MeetingRepository {

    override suspend fun list(): List<Meeting> =
        core.meetingsList().map { dto ->
            Meeting(
                id = dto.id,
                name = dto.name,
                audioPath = dto.audioPath,
                hasTranscript = dto.hasTranscript,
                hasProtocol = dto.hasProtocol,
                createdAt = dto.createdAt,
            )
        }

    override suspend fun protocolLoad(meetingId: String): Protocol? =
        core.protocolLoad(meetingId)?.let { Protocol(it.markdown) }

    override suspend fun protocolGenerate(meetingId: String, templateName: String?): Protocol =
        Protocol(core.protocolGenerate(meetingId, templateName).markdown)

    override suspend fun jobSubmit(audioPath: String, name: String?): JobProgress =
        core.jobSubmit(audioPath, name).toJobProgress()

    override suspend fun jobStatus(jobId: String): JobProgress? =
        core.jobStatus(jobId)?.toJobProgress()

    private fun uniffi.meeting_assistant_ffi.JobDto.toJobProgress() = JobProgress(
        id = id,
        meetingId = meetingId,
        status = when (status.uppercase()) {
            "QUEUED"  -> JobStatus.QUEUED
            "RUNNING" -> JobStatus.RUNNING
            "DONE"    -> JobStatus.DONE
            "FAILED"  -> JobStatus.FAILED
            else      -> JobStatus.UNKNOWN
        },
        attempts = attempts.toInt(),
        lastError = lastError,
    )
}
