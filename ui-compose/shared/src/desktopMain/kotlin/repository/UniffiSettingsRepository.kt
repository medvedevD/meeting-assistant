package repository

import domain.ModelInfo
import domain.RecordingPrefs
import domain.Settings
import domain.SettingsPaths
import domain.TranscriberPrefs
import uniffi.meeting_assistant_ffi.AppCore
import uniffi.meeting_assistant_ffi.RecordingPrefsDto
import uniffi.meeting_assistant_ffi.SettingsDto
import uniffi.meeting_assistant_ffi.SettingsPathsDto
import uniffi.meeting_assistant_ffi.TranscriberPrefsDto

class UniffiSettingsRepository(private val core: AppCore) : SettingsRepository {

    override suspend fun get(): Settings {
        val dto = core.settingsGet()
        return Settings(
            paths = SettingsPaths(
                model = dto.paths.model,
                db = dto.paths.db,
                meetingsDir = dto.paths.meetingsDir,
                prompts = dto.paths.prompts,
            ),
            anthropicApiKey = dto.anthropicApiKey,
            recording = RecordingPrefs(
                source = dto.recording.source,
                echoCancel = dto.recording.echoCancel,
            ),
            defaultTemplate = dto.defaultTemplate,
            transcriber = TranscriberPrefs(
                language = dto.transcriber.language,
                beamSize = dto.transcriber.beamSize.toInt(),
                nThreads = dto.transcriber.nThreads.toInt(),
            ),
        )
    }

    override suspend fun set(settings: Settings) {
        core.settingsSet(
            SettingsDto(
                paths = SettingsPathsDto(
                    model = settings.paths.model,
                    db = settings.paths.db,
                    meetingsDir = settings.paths.meetingsDir,
                    prompts = settings.paths.prompts,
                ),
                anthropicApiKey = settings.anthropicApiKey,
                recording = RecordingPrefsDto(
                    source = settings.recording.source,
                    echoCancel = settings.recording.echoCancel,
                ),
                defaultTemplate = settings.defaultTemplate,
                transcriber = TranscriberPrefsDto(
                    language = settings.transcriber.language,
                    beamSize = settings.transcriber.beamSize.toUInt(),
                    nThreads = settings.transcriber.nThreads.toUInt(),
                ),
            )
        )
    }

    override suspend fun templatesList(): List<String> =
        core.templatesList()

    override suspend fun modelsList(): List<ModelInfo> =
        core.modelsList().map { dto ->
            ModelInfo(
                path = dto.path,
                name = dto.name,
                description = dto.description,
                sizeMb = dto.sizeMb.toInt(),
            )
        }
}
