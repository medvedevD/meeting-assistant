package ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import domain.WHISPER_MODELS
import domain.WhisperModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.meeting_assistant_ffi.AppCore
import uniffi.meeting_assistant_ffi.ModelDownloadCallback
import uniffi.meeting_assistant_ffi.downloadModel

sealed class ModelDownloadState {
    object ChoosingModel : ModelDownloadState()
    data class Downloading(val model: WhisperModel, val downloaded: Long, val total: Long) : ModelDownloadState()
    object Done : ModelDownloadState()
    data class Error(val message: String) : ModelDownloadState()
}

@Composable
fun ModelDownloadScreen(
    core: AppCore,
    onDownloadComplete: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf<ModelDownloadState>(ModelDownloadState.ChoosingModel) }
    var selectedModel by remember { mutableStateOf(WHISPER_MODELS.first { it.id == "medium" }) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 48.dp, vertical = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text("Выберите модель Whisper", style = MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(4.dp))
        Text(
            "Модель используется для транскрипции. Скачивается один раз.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(24.dp))

        when (val s = state) {
            is ModelDownloadState.ChoosingModel -> {
                WHISPER_MODELS.forEach { model ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .selectable(
                                selected = selectedModel == model,
                                onClick = { selectedModel = model },
                            )
                            .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.Top,
                    ) {
                        RadioButton(
                            selected = selectedModel == model,
                            onClick = { selectedModel = model },
                        )
                        Spacer(Modifier.width(8.dp))
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                "${model.displayName}  —  ${model.sizeMb} МБ",
                                style = MaterialTheme.typography.bodyLarge,
                            )
                            Text(
                                model.description,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }

                Spacer(Modifier.height(24.dp))

                Button(
                    onClick = {
                        val model = selectedModel
                        scope.launch(Dispatchers.IO) {
                            state = ModelDownloadState.Downloading(model, 0L, 0L)
                            val callback = object : ModelDownloadCallback {
                                override fun onProgress(bytesDownloaded: Long, totalBytes: Long) {
                                    state = ModelDownloadState.Downloading(model, bytesDownloaded, totalBytes)
                                }
                                override fun onComplete() { state = ModelDownloadState.Done }
                                override fun onError(message: String) {
                                    state = ModelDownloadState.Error(message)
                                }
                            }
                            try {
                                downloadModel(core, model.downloadUrl, null, callback)
                            } catch (e: Exception) {
                                state = ModelDownloadState.Error(e.message ?: "Unknown error")
                            }
                        }
                    },
                ) {
                    Text("Скачать ${selectedModel.displayName}  (${selectedModel.sizeMb} МБ)")
                }
            }

            is ModelDownloadState.Downloading -> {
                Text(
                    "Скачивание ${s.model.displayName}...",
                    style = MaterialTheme.typography.titleMedium,
                )
                Spacer(Modifier.height(16.dp))
                val progress = if (s.total > 0) s.downloaded.toFloat() / s.total else 0f
                val downloadedMb = s.downloaded / (1024 * 1024)
                val totalMb = s.total / (1024 * 1024)
                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                Text("$downloadedMb МБ / $totalMb МБ  (${(progress * 100).toInt()}%)")
                Spacer(Modifier.height(8.dp))
                // Cancellation not implemented — blocking thread continues until done
                OutlinedButton(onClick = {}, enabled = false) {
                    Text("Отмена (не реализовано)")
                }
            }

            is ModelDownloadState.Done -> {
                LaunchedEffect(Unit) { onDownloadComplete() }
                Text("Модель скачана. Запуск приложения...")
                CircularProgressIndicator(modifier = Modifier.padding(top = 16.dp))
            }

            is ModelDownloadState.Error -> {
                Text(
                    "Ошибка: ${s.message}",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
                Spacer(Modifier.height(16.dp))
                Button(onClick = { state = ModelDownloadState.ChoosingModel }) {
                    Text("Попробовать снова")
                }
            }
        }
    }
}
