import WindowPrefs
import kotlin.system.exitProcess
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.arkivanov.decompose.DefaultComponentContext
import com.arkivanov.essenty.lifecycle.LifecycleRegistry
import com.arkivanov.essenty.lifecycle.resume
import com.arkivanov.essenty.lifecycle.stop
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import repository.UniffiDiagnosticsRepository
import repository.UniffiMeetingRepository
import repository.UniffiRecordingRepository
import repository.UniffiSettingsRepository
import ui.AppContent
import ui.navigation.RootComponent
import ui.screens.ModelDownloadScreen
import uniffi.meeting_assistant_ffi.AppConfig
import uniffi.meeting_assistant_ffi.AppException
import uniffi.meeting_assistant_ffi.initCore
import uniffi.meeting_assistant_ffi.modelExists
import uniffi.meeting_assistant_ffi.startWorker
import uniffi.meeting_assistant_ffi.tryAcquireSingleton
import java.io.File
import javax.swing.JOptionPane

private const val WORKER_SHUTDOWN_TIMEOUT_MS = 5_000L

private fun resolveNativeLibPath(): String {
    val osName = System.getProperty("os.name").lowercase()
    val libName = when {
        "linux" in osName -> "libmeeting_assistant_ffi.so"
        "windows" in osName -> "meeting_assistant_ffi.dll"
        "mac" in osName -> "libmeeting_assistant_ffi.dylib"
        else -> error("Unsupported OS: $osName")
    }
    // Production bundle: set by Compose Desktop jpackage launcher
    val resourcesDir = System.getProperty("compose.application.resources.dir")
    if (resourcesDir != null) {
        return File(resourcesDir, libName).absolutePath
    }
    // Development: passed via -Drust.target.dir (set in build.gradle.kts jvmArgs)
    val rustTargetDir = System.getProperty("rust.target.dir")
        ?: error("Pass -Drust.target.dir=<path> to locate $libName")
    return File(rustTargetDir, libName).absolutePath
}

/**
 * Resolves the LLM prompt templates directory. Mirrors [resolveNativeLibPath]:
 * the jpackage bundle exposes `resources/common/prompts` via the resources dir;
 * dev runs receive the repo `prompts/` path via -Dmeeting.prompts.dir.
 */
private fun resolvePromptsDir(): String? {
    // Production bundle: set by Compose Desktop jpackage launcher
    System.getProperty("compose.application.resources.dir")?.let {
        return File(it, "prompts").absolutePath
    }
    // Development: passed via -Dmeeting.prompts.dir (set in build.gradle.kts jvmArgs)
    return System.getProperty("meeting.prompts.dir")
}

fun main() {
    val libPath = resolveNativeLibPath()
    System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", libPath)

    try {
        tryAcquireSingleton()
    } catch (e: AppException.General) {
        JOptionPane.showMessageDialog(
            null,
            "Meeting Assistant is already running.\n\nClose the existing window and try again.",
            "Already Running",
            JOptionPane.WARNING_MESSAGE,
        )
        return
    }

    val core = runBlocking {
        withContext(Dispatchers.IO) {
            initCore(AppConfig(
                modelPath = null,
                dbPath = null,
                meetingsDir = null,
                promptsDir = resolvePromptsDir(),
                anthropicApiKey = null,
            ))
        }
    }

    val workerHandle = runBlocking { startWorker(core) }
    val hasModel = modelExists(core)

    val lifecycle = LifecycleRegistry()

    val root = RootComponent(
        componentContext = DefaultComponentContext(lifecycle),
        meetings = UniffiMeetingRepository(core),
        recording = UniffiRecordingRepository(core),
        settings = UniffiSettingsRepository(core),
        diagnostics = UniffiDiagnosticsRepository(core),
    )

    application {
        val savedX = WindowPrefs.x
        val savedY = WindowPrefs.y
        val windowState = rememberWindowState(
            width = WindowPrefs.width.dp,
            height = WindowPrefs.height.dp,
            position = if (savedX != null && savedY != null)
                WindowPosition(savedX.dp, savedY.dp)
            else
                WindowPosition.PlatformDefault,
        )

        lifecycle.resume()

        Window(
            onCloseRequest = {
                val pos = windowState.position
                WindowPrefs.save(
                    width  = windowState.size.width.value.toInt(),
                    height = windowState.size.height.value.toInt(),
                    x = (pos as? WindowPosition.Absolute)?.x?.value?.toInt(),
                    y = (pos as? WindowPosition.Absolute)?.y?.value?.toInt(),
                )
                lifecycle.stop()
                runBlocking { workerHandle.stopGraceful(WORKER_SHUTDOWN_TIMEOUT_MS.toULong()) }
                exitApplication()
                exitProcess(0)
            },
            title = "Meeting Assistant",
            state = windowState,
        ) {
            if (!hasModel) {
                var modelDownloaded by remember { mutableStateOf(false) }
                if (modelDownloaded) {
                    AppContent(root = root)
                } else {
                    ModelDownloadScreen(
                        core = core,
                        onDownloadComplete = { modelDownloaded = true },
                    )
                }
            } else {
                AppContent(root = root)
            }
        }
    }
}
