package domain

enum class JobStatus { QUEUED, RUNNING, DONE, FAILED, UNKNOWN }

data class JobProgress(
    val id: String,
    val meetingId: String,
    val status: JobStatus,
    val attempts: Int,
    val lastError: String?,
)
