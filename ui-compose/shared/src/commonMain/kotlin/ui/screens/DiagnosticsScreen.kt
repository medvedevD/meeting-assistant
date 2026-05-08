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
import domain.DiagnosticsInfo
import ui.navigation.RootComponent

@Composable
fun DiagnosticsScreen(root: RootComponent) {
    var info by remember { mutableStateOf<DiagnosticsInfo?>(null) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        try {
            info = root.diagnostics.get()
        } catch (e: Exception) {
            error = e.message
        } finally {
            loading = false
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        DiagnosticsToolbar(onBack = { root.onBackToList() })
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        when {
            loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            error != null -> Box(
                Modifier.fillMaxSize().padding(32.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text("Ошибка: $error", color = MaterialTheme.colorScheme.error)
            }
            info != null -> DiagnosticsContent(info!!)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DiagnosticsToolbar(onBack: () -> Unit) {
    TopAppBar(
        title = { Text("Диагностика", style = MaterialTheme.typography.titleMedium) },
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
private fun DiagnosticsContent(info: DiagnosticsInfo) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SectionTitle("Система")
        DiagRow("ОС", info.os)
        DiagRow("Архитектура", info.arch)
        DiagRow("Версия", info.appVersion)
        DiagRow("CPAL Host", info.cpalHost)
        DiagRow("FFmpeg", if (info.ffmpegOk) "OK" else "Не найден")
        DiagRow("API Key", if (info.hasAnthropicKey) "Установлен" else "Отсутствует")

        SectionTitle("Аудиоустройства")
        if (info.inputDevices.isEmpty()) {
            Text("Нет устройств ввода", style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            info.inputDevices.forEach { d ->
                DiagRow(if (d.isDefault) "${d.name} (по умолч.)" else d.name, "вход")
            }
        }

        SectionTitle("Пути")
        DiagRow("Модель", info.paths.model.path)
        DiagRow("База данных", info.paths.db.path)
        DiagRow("Записи", info.paths.recordings.path)
        DiagRow("Промпты", info.paths.prompts.path)
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
    )
}

@Composable
private fun DiagRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurface)
    }
}
