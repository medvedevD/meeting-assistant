package ui.screens

import androidx.compose.animation.core.*
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import ui.navigation.RootComponent
import ui.util.showAudioFilePicker
import viewmodel.RecordingUiState

@Composable
fun NewRecordingScreen(root: RootComponent) {
    val state by root.recordingViewModel.state.collectAsState()
    val scope = rememberCoroutineScope()

    // Navigate away when recording is saved — go straight to protocol generation.
    LaunchedEffect(state) {
        if (state is RecordingUiState.Done) {
            val done = state as RecordingUiState.Done
            root.recordingViewModel.reset()
            root.meetingListViewModel.refresh()
            root.onGenerateProtocol(done.handle.id, autoStart = true)
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        RecordingToolbar(
            canGoBack = state is RecordingUiState.Idle || state is RecordingUiState.Error,
            onBack = { root.onBackToList() },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        when (val s = state) {
            is RecordingUiState.Idle -> IdlePane(
                onStart = { name, source, echoCancel ->
                    root.recordingViewModel.start(name, source, echoCancel)
                },
                onImport = { name ->
                    scope.launch {
                        val path = withContext(Dispatchers.Main) { showAudioFilePicker() }
                            ?: return@launch
                        try {
                            val job = root.meetings.jobSubmit(path, name.ifBlank { null })
                            root.meetingListViewModel.refresh()
                            root.onMeetingSelected(job.meetingId)
                        } catch (_: Exception) { /* ignore — user still on screen */ }
                    }
                },
            )
            is RecordingUiState.Recording -> RecordingPane(
                name = s.name,
                elapsedSeconds = s.elapsedSeconds,
                onStop = { root.recordingViewModel.stop() },
            )
            is RecordingUiState.Stopping -> StoppingPane()
            is RecordingUiState.Done -> StoppingPane() // briefly shown before LaunchedEffect navigates
            is RecordingUiState.Error -> ErrorPane(
                message = s.message,
                onDismiss = { root.recordingViewModel.reset() },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RecordingToolbar(canGoBack: Boolean, onBack: () -> Unit) {
    TopAppBar(
        title = { Text("Новая запись", style = MaterialTheme.typography.titleMedium) },
        navigationIcon = {
            if (canGoBack) {
                IconButton(onClick = onBack) {
                    Icon(Icons.Default.ArrowBack, contentDescription = "Назад")
                }
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    )
}

private val audioSources = listOf(
    "mic"    to "Микрофон",
    "system" to "Система",
    "mixed"  to "Оба",
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun IdlePane(
    onStart: (name: String, source: String, echoCancel: Boolean) -> Unit,
    onImport: (name: String) -> Unit,
) {
    var name by remember { mutableStateOf("") }
    var source by remember { mutableStateOf("mixed") }
    var echoCancel by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 40.dp, vertical = 32.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("Название встречи") },
            placeholder = { Text("Встреча с командой") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                "Источник звука",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                audioSources.forEachIndexed { index, (value, label) ->
                    SegmentedButton(
                        shape = SegmentedButtonDefaults.itemShape(index, audioSources.size),
                        selected = source == value,
                        onClick = { source = value },
                    ) {
                        Text(label)
                    }
                }
            }
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column {
                Text("Подавление эха", style = MaterialTheme.typography.bodyMedium)
                Text(
                    "Рекомендуется при записи через микрофон",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = echoCancel, onCheckedChange = { echoCancel = it })
        }

        Spacer(Modifier.weight(1f))

        Button(
            onClick = { onStart(name, source, echoCancel) },
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) {
            Text("Начать запись")
        }

        OutlinedButton(
            onClick = { onImport(name) },
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) {
            Text("Импортировать аудиофайл…")
        }
    }
}

@Composable
private fun RecordingPane(name: String, elapsedSeconds: Int, onStop: () -> Unit) {
    val pulse = rememberInfiniteTransition(label = "pulse")
    val alpha by pulse.animateFloat(
        initialValue = 1f,
        targetValue = 0.3f,
        animationSpec = infiniteRepeatable(
            animation = tween(800, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "pulseAlpha",
    )

    Column(
        modifier = Modifier.fillMaxSize().padding(40.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            "●  ЗАПИСЬ",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.alpha(alpha),
        )
        Spacer(Modifier.height(16.dp))
        Text(
            formatElapsed(elapsedSeconds),
            style = MaterialTheme.typography.displayMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            name,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(48.dp))
        Button(
            onClick = onStop,
            modifier = Modifier.width(220.dp).height(48.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.error,
            ),
        ) {
            Text("Остановить запись")
        }
    }
}

@Composable
private fun StoppingPane() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            CircularProgressIndicator()
            Spacer(Modifier.height(16.dp))
            Text(
                "Сохранение записи…",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ErrorPane(message: String, onDismiss: () -> Unit) {
    Box(modifier = Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                "Ошибка: $message",
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(16.dp))
            OutlinedButton(onClick = onDismiss) { Text("Попробовать снова") }
        }
    }
}

private fun formatElapsed(seconds: Int): String {
    val m = seconds / 60
    val s = seconds % 60
    return "%d:%02d".format(m, s)
}
