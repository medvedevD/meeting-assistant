import WindowPrefs
import kotlin.system.exitProcess
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
import uniffi.meeting_assistant_ffi.AppConfig
import uniffi.meeting_assistant_ffi.AppException
import uniffi.meeting_assistant_ffi.initCore
import uniffi.meeting_assistant_ffi.startWorker
import uniffi.meeting_assistant_ffi.tryAcquireSingleton
import java.io.File
import javax.swing.JOptionPane

private const val WORKER_SHUTDOWN_TIMEOUT_MS = 5_000L

fun main() {
    val rustTargetDir = System.getProperty("rust.target.dir")
        ?: error("Pass -Drust.target.dir=<path> to locate libmeeting_assistant_ffi.so")
    val soPath = File(rustTargetDir, "libmeeting_assistant_ffi.so").absolutePath
    System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", soPath)

    // Single-instance check before loading the Whisper model.
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

    // Init AppCore on IO thread (loads Whisper model from disk).
    val core = runBlocking {
        withContext(Dispatchers.IO) {
            initCore(AppConfig(
                modelPath = null,
                dbPath = null,
                meetingsDir = null,
                promptsDir = null,
                anthropicApiKey = null,
            ))
        }
    }

    val workerHandle = runBlocking { startWorker(core) }

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
                // Let the worker finish its current job; abort after timeout.
                runBlocking { workerHandle.stopGraceful(WORKER_SHUTDOWN_TIMEOUT_MS.toULong()) }
                exitApplication()
                exitProcess(0)
            },
            title = "Meeting Assistant",
            state = windowState,
        ) {
            AppContent(root = root)
        }
    }
}
