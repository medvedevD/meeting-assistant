package repository

import domain.*
import uniffi.meeting_assistant_ffi.AppCore

class UniffiDiagnosticsRepository(private val core: AppCore) : DiagnosticsRepository {

    override suspend fun get(): DiagnosticsInfo {
        val dto = core.diagnostics()
        return DiagnosticsInfo(
            os = dto.os,
            arch = dto.arch,
            appVersion = dto.appVersion,
            cpalHost = dto.cpalHost,
            inputDevices = dto.inputDevices.map { DeviceInfo(it.name, it.isDefault) },
            outputDevices = dto.outputDevices.map { DeviceInfo(it.name, it.isDefault) },
            paths = DiagnosticsPaths(
                model = dto.paths.model.let { PathInfo(it.path, it.exists, it.writable, it.sizeBytes) },
                db = dto.paths.db.let { PathInfo(it.path, it.exists, it.writable, it.sizeBytes) },
                meetingsDir = dto.paths.meetingsDir.let { PathInfo(it.path, it.exists, it.writable, it.sizeBytes) },
                prompts = dto.paths.prompts.let { PathInfo(it.path, it.exists, it.writable, it.sizeBytes) },
            ),
            hasAnthropicKey = dto.hasAnthropicKey,
            ffmpegOk = dto.ffmpegOk,
        )
    }

    override suspend fun logs(): List<String> = core.diagnosticsLogs()

    override suspend fun openPath(path: String) = core.openPath(path)
}
