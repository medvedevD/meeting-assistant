package repository

data class RecordingHandle(val id: String, val audioPath: String, val name: String)

interface RecordingRepository {
    suspend fun start(name: String, source: String, echoCancel: Boolean): RecordingHandle
    suspend fun stop(recordingId: String): RecordingHandle
}
