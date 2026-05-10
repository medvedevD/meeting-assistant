package repository

import domain.RecordingPrefs
import domain.Settings
import domain.SettingsPaths
import uniffi.meeting_assistant_ffi.AppCore
import uniffi.meeting_assistant_ffi.RecordingPrefsDto
import uniffi.meeting_assistant_ffi.SettingsDto
import uniffi.meeting_assistant_ffi.SettingsPathsDto

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
            )
        )
    }

    override suspend fun templatesList(): List<String> =
        core.templatesList()
}
