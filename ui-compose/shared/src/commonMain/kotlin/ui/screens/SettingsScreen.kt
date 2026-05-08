package ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import domain.Settings
import kotlinx.coroutines.launch
import ui.navigation.RootComponent

@Composable
fun SettingsScreen(root: RootComponent) {
    var settings by remember { mutableStateOf<Settings?>(null) }
    var loading by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        try {
            settings = root.settings.get()
        } finally {
            loading = false
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        SettingsToolbar(onBack = { root.onBackToList() })
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        if (loading) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        } else {
            val s = settings
            if (s != null) {
                SettingsForm(settings = s, onSave = { updated ->
                    scope.launch {
                        root.settings.set(updated)
                        settings = updated
                    }
                })
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsToolbar(onBack: () -> Unit) {
    TopAppBar(
        title = { Text("Настройки", style = MaterialTheme.typography.titleMedium) },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.Default.ArrowBack, contentDescription = "Назад")
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    )
}

@Composable
private fun SettingsForm(settings: Settings, onSave: (Settings) -> Unit) {
    var apiKey by remember(settings) { mutableStateOf(settings.anthropicApiKey ?: "") }
    var recordingsDir by remember(settings) { mutableStateOf(settings.paths.recordings ?: "") }
    var modelPath by remember(settings) { mutableStateOf(settings.paths.model ?: "") }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("API", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)

        OutlinedTextField(
            value = apiKey,
            onValueChange = { apiKey = it },
            label = { Text("Anthropic API Key") },
            placeholder = { Text("sk-ant-...") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )

        Text("Пути", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)

        OutlinedTextField(
            value = recordingsDir,
            onValueChange = { recordingsDir = it },
            label = { Text("Папка записей") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )

        OutlinedTextField(
            value = modelPath,
            onValueChange = { modelPath = it },
            label = { Text("Путь к модели Whisper") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
        )

        Spacer(Modifier.height(8.dp))

        Button(
            onClick = {
                onSave(
                    settings.copy(
                        anthropicApiKey = apiKey.ifBlank { null },
                        paths = settings.paths.copy(
                            recordings = recordingsDir.ifBlank { null },
                            model = modelPath.ifBlank { null },
                        ),
                    )
                )
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Сохранить")
        }
    }
}
