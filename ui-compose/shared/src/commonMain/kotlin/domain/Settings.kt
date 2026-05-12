package domain

data class ModelInfo(
    val path: String,
    val name: String,
    val description: String,
    val sizeMb: Int,
)

data class SettingsPaths(
    val model: String?,
    val db: String?,
    val meetingsDir: String?,
    val prompts: String?,
)

data class RecordingPrefs(
    val source: String,
    val echoCancel: Boolean,
)

data class TranscriberPrefs(
    val language: String,
    val beamSize: Int = 1,
    val nThreads: Int = 0,
) {
    companion object {
        val Default = TranscriberPrefs(language = "ru", beamSize = 1, nThreads = 0)
    }
}

data class Settings(
    val paths: SettingsPaths,
    val anthropicApiKey: String?,
    val recording: RecordingPrefs,
    val defaultTemplate: String?,
    val transcriber: TranscriberPrefs = TranscriberPrefs.Default,
)
