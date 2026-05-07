import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import uniffi.meeting_assistant_ffi.AppConfig
import java.io.File

fun main() {
    val rustTargetDir = System.getProperty("rust.target.dir")
        ?: error("Pass -Drust.target.dir=<path> to locate libmeeting_assistant_ffi.so")
    val soPath = File(rustTargetDir, "libmeeting_assistant_ffi.so").absolutePath
    System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", soPath)

    application {
        val windowState = rememberWindowState(
            width = 1280.dp,
            height = 800.dp,
            position = WindowPosition(Alignment.Center)
        )
        Window(
            onCloseRequest = ::exitApplication,
            title = "Meeting Assistant",
            state = windowState,
        ) {
            MaterialTheme {
                Text("Meeting Assistant — Stage 1: FFI ready")
            }
        }
    }
}
