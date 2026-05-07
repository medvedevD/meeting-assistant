import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import uniffi.meeting_assistant_ffi.ping
import java.io.File

fun main() {
    val rustTargetDir = System.getProperty("rust.target.dir")
        ?: error("Pass -Drust.target.dir=<path> to locate libmeeting_assistant_ffi.so")
    val soPath = File(rustTargetDir, "libmeeting_assistant_ffi.so").absolutePath
    System.setProperty("uniffi.component.meeting_assistant_ffi.libraryOverride", soPath)

    val result = ping()
    println("ping() = $result")

    application {
        val windowState = rememberWindowState(
            width = 400.dp,
            height = 200.dp,
            position = WindowPosition(Alignment.Center)
        )
        Window(
            onCloseRequest = ::exitApplication,
            title = result,
            state = windowState,
        ) {
            MaterialTheme {
                Text("Rust says: $result")
            }
        }
    }
}
