package domain

data class SettingsPaths(
    val model: String?,
    val db: String?,
    val recordings: String?,
    val prompts: String?,
)

data class RecordingPrefs(
    val source: String,
    val echoCancel: Boolean,
)

data class Settings(
    val paths: SettingsPaths,
    val anthropicApiKey: String?,
    val recording: RecordingPrefs,
    val defaultTemplate: String?,
)
