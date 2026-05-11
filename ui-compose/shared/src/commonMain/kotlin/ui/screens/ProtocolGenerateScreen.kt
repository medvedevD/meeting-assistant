package ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import domain.Meeting
import ui.navigation.RootComponent
import viewmodel.GenerateState
import viewmodel.ProtocolGenerateViewModel

@Composable
fun ProtocolGenerateScreen(meetingId: String, autoStart: Boolean = false, root: RootComponent) {
    val vm = remember(meetingId) { ProtocolGenerateViewModel(root.meetings) }
    DisposableEffect(vm) { onDispose { vm.onDestroy() } }

    val state by vm.state.collectAsState()

    var meeting by remember { mutableStateOf<Meeting?>(null) }
    var templates by remember { mutableStateOf<List<String>>(emptyList()) }
    var selectedTemplate by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(meetingId) {
        val all = runCatching { root.meetings.list() }.getOrDefault(emptyList())
        meeting = all.firstOrNull { it.id == meetingId }
        templates = runCatching { root.settings.templatesList() }.getOrDefault(emptyList())
        if (selectedTemplate == null) {
            selectedTemplate = templates.firstOrNull()
        }
    }

    // Auto-start generation after recording — skip the idle screen.
    LaunchedEffect(meeting) {
        val m = meeting ?: return@LaunchedEffect
        if (autoStart && state is GenerateState.Idle) {
            vm.generate(
                meetingId = meetingId,
                audioPath = m.audioPath,
                hasTranscript = m.hasTranscript,
                templateName = selectedTemplate,
                meetingName = m.name,
            )
        }
    }

    LaunchedEffect(state) {
        if (state is GenerateState.Done) {
            root.meetingListViewModel.refresh()
            root.onMeetingSelected(meetingId)
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        GenerateToolbar(
            canGoBack = state is GenerateState.Idle
                || state is GenerateState.TranscriptFailed
                || state is GenerateState.Failed,
            onBack = { root.onMeetingSelected(meetingId) },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        when (val s = state) {
            is GenerateState.Idle -> IdleGeneratePane(
                meeting = meeting,
                templates = templates,
                selectedTemplate = selectedTemplate,
                onTemplateSelected = { selectedTemplate = it },
                onGenerate = {
                    val m = meeting ?: return@IdleGeneratePane
                    vm.generate(
                        meetingId = meetingId,
                        audioPath = m.audioPath,
                        hasTranscript = m.hasTranscript,
                        templateName = selectedTemplate,
                        meetingName = m.name,
                    )
                },
            )

            is GenerateState.Transcribing -> ProgressPane(
                title = "Транскрипция аудио…",
                subtitle = "Попытка ${s.attempts + 1} · это может занять несколько минут",
            )

            is GenerateState.TranscriptFailed -> ErrorGeneratePane(
                message = s.error,
                onRetry = {
                    vm.reset()
                    val m = meeting ?: return@ErrorGeneratePane
                    vm.generate(
                        meetingId = meetingId,
                        audioPath = m.audioPath,
                        hasTranscript = false,
                        templateName = selectedTemplate,
                        meetingName = m.name,
                    )
                },
            )

            is GenerateState.GeneratingProtocol -> ProgressPane(
                title = "Генерация протокола…",
                subtitle = "Claude обрабатывает транскрипт",
            )

            is GenerateState.Done -> ProgressPane(
                title = "Готово!",
                subtitle = "Открываем протокол…",
            )

            is GenerateState.Failed -> ErrorGeneratePane(
                message = s.error,
                onRetry = {
                    vm.reset()
                    val m = meeting ?: return@ErrorGeneratePane
                    vm.generate(
                        meetingId = meetingId,
                        audioPath = m.audioPath,
                        hasTranscript = m.hasTranscript,
                        templateName = selectedTemplate,
                        meetingName = m.name,
                    )
                },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun GenerateToolbar(canGoBack: Boolean, onBack: () -> Unit) {
    TopAppBar(
        title = { Text("Генерация протокола", style = MaterialTheme.typography.titleMedium) },
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun IdleGeneratePane(
    meeting: Meeting?,
    templates: List<String>,
    selectedTemplate: String?,
    onTemplateSelected: (String?) -> Unit,
    onGenerate: () -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 40.dp, vertical = 32.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        if (meeting != null) {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    meeting.name,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (!meeting.hasTranscript) {
                        InfoChip("Транскрипция будет запущена автоматически")
                    } else {
                        InfoChip("Транскрипт готов")
                    }
                }
            }
        }

        if (templates.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Шаблон протокола",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ExposedDropdownMenuBox(
                    expanded = expanded,
                    onExpandedChange = { expanded = it },
                ) {
                    OutlinedTextField(
                        value = selectedTemplate ?: "По умолчанию",
                        onValueChange = {},
                        readOnly = true,
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
                        modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
                    )
                    ExposedDropdownMenu(
                        expanded = expanded,
                        onDismissRequest = { expanded = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text("По умолчанию") },
                            onClick = {
                                onTemplateSelected(null)
                                expanded = false
                            },
                        )
                        templates.forEach { tpl ->
                            DropdownMenuItem(
                                text = { Text(tpl) },
                                onClick = {
                                    onTemplateSelected(tpl)
                                    expanded = false
                                },
                            )
                        }
                    }
                }
            }
        }

        Spacer(Modifier.weight(1f))

        Button(
            onClick = onGenerate,
            modifier = Modifier.fillMaxWidth().height(48.dp),
            enabled = meeting != null,
        ) {
            Text("Сгенерировать протокол")
        }
    }
}

@Composable
private fun InfoChip(label: String) {
    Surface(
        color = MaterialTheme.colorScheme.secondaryContainer,
        shape = MaterialTheme.shapes.small,
    ) {
        Text(
            label,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
        )
    }
}

@Composable
private fun ProgressPane(title: String, subtitle: String) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CircularProgressIndicator()
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ErrorGeneratePane(message: String, onRetry: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                "Ошибка: $message",
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
            Button(onClick = onRetry) { Text("Повторить") }
        }
    }
}
