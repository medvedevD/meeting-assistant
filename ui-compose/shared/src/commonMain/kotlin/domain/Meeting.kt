package domain

data class Meeting(
    val id: String,
    val name: String,
    val audioPath: String,
    val hasTranscript: Boolean,
    val hasProtocol: Boolean,
    val createdAt: Long,
)
