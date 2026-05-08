package domain

data class PathInfo(
    val path: String,
    val exists: Boolean,
    val writable: Boolean,
    val sizeBytes: ULong?,
)

data class DeviceInfo(
    val name: String,
    val isDefault: Boolean,
)

data class DiagnosticsPaths(
    val model: PathInfo,
    val db: PathInfo,
    val recordings: PathInfo,
    val prompts: PathInfo,
)

data class DiagnosticsInfo(
    val os: String,
    val arch: String,
    val appVersion: String,
    val cpalHost: String,
    val inputDevices: List<DeviceInfo>,
    val outputDevices: List<DeviceInfo>,
    val paths: DiagnosticsPaths,
    val hasAnthropicKey: Boolean,
    val ffmpegOk: Boolean,
)
