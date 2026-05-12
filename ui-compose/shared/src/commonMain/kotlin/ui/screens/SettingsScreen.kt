package ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import domain.ModelInfo
import domain.RecordingPrefs
import domain.Settings
import domain.SettingsPaths
import domain.TranscriberPrefs
import kotlinx.coroutines.launch
import ui.navigation.RootComponent

@Composable
fun SettingsScreen(root: RootComponent) {
    var settings by remember { mutableStateOf<Settings?>(null) }
    var templates by remember { mutableStateOf<List<String>>(emptyList()) }
    var models by remember { mutableStateOf<List<ModelInfo>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }

    LaunchedEffect(Unit) {
        try {
            settings = root.settings.get()
            templates = root.settings.templatesList()
            models = root.settings.modelsList()
        } finally {
            loading = false
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding),
        ) {
            SettingsToolbar(onBack = { root.onBackToList() })
            HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

            if (loading) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else {
                val s = settings
                if (s != null) {
                    SettingsForm(
                        settings = s,
                        templates = templates,
                        models = models,
                        onSave = { updated ->
                            scope.launch {
                                try {
                                    root.settings.set(updated)
                                    settings = updated
                                    snackbar.showSnackbar("Настройки сохранены")
                                } catch (e: Exception) {
                                    snackbar.showSnackbar("Ошибка: ${e.message}")
                                }
                            }
                        },
                    )
                }
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
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsForm(
    settings: Settings,
    templates: List<String>,
    models: List<ModelInfo>,
    onSave: (Settings) -> Unit,
) {
    // API
    var apiKey by remember { mutableStateOf("") }
    var apiKeyChanged by remember { mutableStateOf(false) }
    var apiKeyVisible by remember { mutableStateOf(false) }

    // Paths
    var modelPath by remember(settings) { mutableStateOf(settings.paths.model ?: "") }
    var modelDropdownExpanded by remember { mutableStateOf(false) }
    var showCustomModelPath by remember { mutableStateOf(false) }
    LaunchedEffect(models) {
        if (models.isNotEmpty() && modelPath.isNotEmpty() && models.none { it.path == modelPath }) {
            showCustomModelPath = true
        }
    }
    var dbPath by remember(settings) { mutableStateOf(settings.paths.db ?: "") }
    var meetingsDir by remember(settings) { mutableStateOf(settings.paths.meetingsDir ?: "") }
    var promptsDir by remember(settings) { mutableStateOf(settings.paths.prompts ?: "") }

    // Recording
    var recSource by remember(settings) { mutableStateOf(settings.recording.source) }
    var echoCancel by remember(settings) { mutableStateOf(settings.recording.echoCancel) }

    // Template
    var defaultTemplate by remember(settings) { mutableStateOf(settings.defaultTemplate) }
    var templateExpanded by remember { mutableStateOf(false) }

    // Transcriber
    var transcLang by remember(settings) { mutableStateOf(settings.transcriber.language) }
    var transcBeamSize by remember(settings) { mutableStateOf(settings.transcriber.beamSize) }
    var transcThreads by remember(settings) { mutableStateOf(settings.transcriber.nThreads) }

    // Validation
    fun String.isValidPath() = isBlank() || startsWith("/") || startsWith("~")
    val pathsValid = modelPath.isValidPath() && dbPath.isValidPath()
        && meetingsDir.isValidPath() && promptsDir.isValidPath()
    val apiKeyValid = !apiKeyChanged || apiKey.isBlank() || apiKey.startsWith("sk-ant-")

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 32.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // ── Anthropic ───────────────────────────────────────────────────────
        SectionHeader("Anthropic")

        OutlinedTextField(
            value = apiKey,
            onValueChange = { apiKey = it; apiKeyChanged = true },
            label = { Text("API Key") },
            placeholder = { Text("sk-ant-…") },
            singleLine = true,
            isError = !apiKeyValid,
            visualTransformation = if (apiKeyVisible) VisualTransformation.None
                                   else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            trailingIcon = {
                IconButton(onClick = { apiKeyVisible = !apiKeyVisible }) {
                    Icon(
                        if (apiKeyVisible) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                        contentDescription = if (apiKeyVisible) "Скрыть" else "Показать",
                    )
                }
            },
            supportingText = {
                if (!apiKeyValid) Text("Ключ должен начинаться с sk-ant-", color = MaterialTheme.colorScheme.error)
                else Text("Ключ хранится безопасно и не отображается при загрузке")
            },
            modifier = Modifier.fillMaxWidth(),
        )

        // ── Запись ──────────────────────────────────────────────────────────
        Spacer(Modifier.height(8.dp))
        SectionHeader("Запись")

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
                        selected = recSource == value,
                        onClick = { recSource = value },
                    ) { Text(label) }
                }
            }
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        ) {
            Text("Подавление эха", style = MaterialTheme.typography.bodyMedium)
            Switch(checked = echoCancel, onCheckedChange = { echoCancel = it })
        }

        // ── Шаблон ──────────────────────────────────────────────────────────
        if (templates.isNotEmpty()) {
            Spacer(Modifier.height(8.dp))
            SectionHeader("Шаблон по умолчанию")

            ExposedDropdownMenuBox(
                expanded = templateExpanded,
                onExpandedChange = { templateExpanded = it },
            ) {
                OutlinedTextField(
                    value = defaultTemplate ?: "По умолчанию",
                    onValueChange = {},
                    readOnly = true,
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = templateExpanded) },
                    modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
                )
                ExposedDropdownMenu(
                    expanded = templateExpanded,
                    onDismissRequest = { templateExpanded = false },
                ) {
                    DropdownMenuItem(
                        text = { Text("По умолчанию") },
                        onClick = { defaultTemplate = null; templateExpanded = false },
                    )
                    templates.forEach { tpl ->
                        DropdownMenuItem(
                            text = { Text(tpl) },
                            onClick = { defaultTemplate = tpl; templateExpanded = false },
                        )
                    }
                }
            }
        }

        // ── Транскрипция ─────────────────────────────────────────────────────
        Spacer(Modifier.height(8.dp))
        SectionHeader("Транскрипция")

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                "Язык",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                transcLanguages.forEachIndexed { index, (value, label) ->
                    SegmentedButton(
                        shape = SegmentedButtonDefaults.itemShape(index, transcLanguages.size),
                        selected = transcLang == value,
                        onClick = { transcLang = value },
                    ) { Text(label) }
                }
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    "Точность распознавания",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    transcBeamSize.toString(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Slider(
                value = transcBeamSize.toFloat(),
                onValueChange = { transcBeamSize = it.toInt() },
                valueRange = 1f..5f,
                steps = 3,
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                when (transcBeamSize) {
                    1    -> "Самый быстрый режим. Берёт первый подходящий вариант."
                    2    -> "Сравнивает 2 варианта расшифровки, выбирает лучший."
                    3    -> "Сравнивает 3 варианта. Баланс скорости и точности."
                    4    -> "Сравнивает 4 варианта. Заметно медленнее, точнее."
                    else -> "Сравнивает 5 вариантов. Максимальная точность, ~5x медленнее чем 1."
                },
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    "CPU потоки",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    if (transcThreads == 0) "Авто" else transcThreads.toString(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Slider(
                value = transcThreads.toFloat(),
                onValueChange = { transcThreads = it.toInt() },
                valueRange = 0f..16f,
                steps = 15,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        // ── Пути ────────────────────────────────────────────────────────────
        Spacer(Modifier.height(8.dp))
        SectionHeader("Хранилище")

        PathField(value = meetingsDir, onValueChange = { meetingsDir = it }, label = "Папка встреч")
        PathField(value = dbPath, onValueChange = { dbPath = it }, label = "База данных (SQLite)")

        Spacer(Modifier.height(8.dp))
        SectionHeader("Модель")

        if (models.isEmpty()) {
            PathField(value = modelPath, onValueChange = { modelPath = it }, label = "Путь к модели Whisper (.bin)")
        } else {
            val selectedModel = models.find { it.path == modelPath }
            ExposedDropdownMenuBox(
                expanded = modelDropdownExpanded,
                onExpandedChange = { modelDropdownExpanded = it },
            ) {
                OutlinedTextField(
                    value = when {
                        selectedModel != null -> "${selectedModel.name}  ·  ${selectedModel.sizeMb} MB"
                        showCustomModelPath   -> "Другой путь…"
                        else                 -> "Не выбрана"
                    },
                    onValueChange = {},
                    readOnly = true,
                    label = { Text("Модель Whisper") },
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = modelDropdownExpanded) },
                    modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
                )
                ExposedDropdownMenu(
                    expanded = modelDropdownExpanded,
                    onDismissRequest = { modelDropdownExpanded = false },
                ) {
                    models.forEach { model ->
                        DropdownMenuItem(
                            text = {
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    Text(
                                        "${model.name}  ·  ${model.sizeMb} MB",
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                    Text(
                                        model.description,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            },
                            onClick = {
                                modelPath = model.path
                                showCustomModelPath = false
                                modelDropdownExpanded = false
                            },
                        )
                    }
                    DropdownMenuItem(
                        text = { Text("Другой путь…", style = MaterialTheme.typography.bodyMedium) },
                        onClick = {
                            showCustomModelPath = true
                            modelDropdownExpanded = false
                        },
                    )
                }
            }
            if (showCustomModelPath) {
                Spacer(Modifier.height(4.dp))
                PathField(value = modelPath, onValueChange = { modelPath = it }, label = "Путь к модели Whisper (.bin)")
            }
        }
        PathField(value = promptsDir, onValueChange = { promptsDir = it }, label = "Папка промптов")

        // ── Save ────────────────────────────────────────────────────────────
        Spacer(Modifier.height(16.dp))

        Button(
            onClick = {
                val updatedApiKey = when {
                    !apiKeyChanged -> null          // unchanged — pass None
                    apiKey.isBlank() -> ""          // clear key — pass Some("")
                    else -> apiKey                  // new key
                }
                onSave(
                    Settings(
                        paths = SettingsPaths(
                            model = modelPath.ifBlank { null },
                            db = dbPath.ifBlank { null },
                            meetingsDir = meetingsDir.ifBlank { null },
                            prompts = promptsDir.ifBlank { null },
                        ),
                        anthropicApiKey = updatedApiKey,
                        recording = RecordingPrefs(source = recSource, echoCancel = echoCancel),
                        defaultTemplate = defaultTemplate,
                        transcriber = TranscriberPrefs(language = transcLang, beamSize = transcBeamSize, nThreads = transcThreads),
                    )
                )
            },
            enabled = pathsValid && apiKeyValid,
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) {
            Text("Сохранить")
        }
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 8.dp),
    )
}

@Composable
private fun PathField(value: String, onValueChange: (String) -> Unit, label: String) {
    val invalid = value.isNotBlank() && !value.startsWith("/") && !value.startsWith("~")
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text("/home/user/…") },
        singleLine = true,
        isError = invalid,
        supportingText = if (invalid) ({ Text("Укажите абсолютный путь (/…) или ~/…", color = MaterialTheme.colorScheme.error) }) else null,
        modifier = Modifier.fillMaxWidth(),
    )
}

private val audioSources = listOf(
    "mic"    to "Микрофон",
    "system" to "Система",
    "mixed"  to "Оба",
)

private val transcLanguages = listOf(
    "ru"   to "Русский",
    "en"   to "English",
    "auto" to "Авто",
)

