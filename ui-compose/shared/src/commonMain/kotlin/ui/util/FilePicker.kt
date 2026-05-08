package ui.util

/** Shows a native OS file picker filtered to common audio formats.
 *  Returns the selected absolute path, or null if cancelled. */
expect fun showAudioFilePicker(title: String = "Выберите аудиофайл"): String?
