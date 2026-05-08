import WindowPrefs
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
import uniffi.meeting_assistant_ffi.initCore
import uniffi.meeting_assistant_ffi.startWorker
import java.io.File

fun main() {
    val rustTargetDir = System.getProperty("rust.target.dir")
        ?: error("Pass -Drust.target.dir=<path> to locate libmeeting_assistant_ffi.so")
    val soPath = File(rustTargetDir, "libmeeting_assistant_ffi.so").absolutePath
    System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", soPath)

    // Init AppCore on IO thread (loads Whisper model from disk).
    val core = runBlocking {
        withContext(Dispatchers.IO) {
            initCore(AppConfig(
                modelPath = null,
                dbPath = null,
                recordingsDir = null,
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
                workerHandle.stop()
                exitApplication()
            },
            title = "Meeting Assistant",
            state = windowState,
        ) {
            AppContent(root = root)
        }
    }
}
