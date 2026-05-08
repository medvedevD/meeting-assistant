package repository

import uniffi.meeting_assistant_ffi.AppCore

class UniffiRecordingRepository(private val core: AppCore) : RecordingRepository {

    override suspend fun start(name: String, source: String, echoCancel: Boolean): RecordingHandle {
        val dto = core.recordingStart(name = name, source = source, echoCancel = echoCancel)
        return RecordingHandle(id = dto.id, audioPath = dto.audioPath, name = dto.name)
    }

    override suspend fun stop(recordingId: String): RecordingHandle {
        val dto = core.recordingStop(recordingId)
        return RecordingHandle(id = dto.id, audioPath = dto.audioPath, name = dto.name)
    }
}
