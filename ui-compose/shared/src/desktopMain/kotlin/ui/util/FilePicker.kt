package ui.util

import java.awt.FileDialog
import java.awt.Frame
import java.io.FilenameFilter

actual fun showAudioFilePicker(title: String): String? {
    val dialog = FileDialog(null as Frame?, title, FileDialog.LOAD)
    dialog.filenameFilter = FilenameFilter { _, name ->
        val lower = name.lowercase()
        lower.endsWith(".wav") || lower.endsWith(".mp3") || lower.endsWith(".m4a")
            || lower.endsWith(".ogg") || lower.endsWith(".flac") || lower.endsWith(".opus")
    }
    dialog.isVisible = true
    val dir = dialog.directory ?: return null
    val file = dialog.file ?: return null
    return "$dir$file"
}
