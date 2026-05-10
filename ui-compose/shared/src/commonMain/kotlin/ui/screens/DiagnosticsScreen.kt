package ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import domain.DeviceInfo
import domain.DiagnosticsInfo
import domain.PathInfo
import kotlinx.coroutines.launch
import ui.navigation.RootComponent

@Composable
fun DiagnosticsScreen(root: RootComponent) {
    var info by remember { mutableStateOf<DiagnosticsInfo?>(null) }
    var logs by remember { mutableStateOf<List<String>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var refreshKey by remember { mutableStateOf(0) }
    var selectedTab by remember { mutableStateOf(0) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(refreshKey) {
        loading = true
        error = null
        try {
            info = root.diagnostics.get()
            logs = root.diagnostics.logs()
        } catch (e: Exception) {
            error = e.message
        } finally {
            loading = false
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        DiagnosticsToolbar(
            onBack = { root.onBackToList() },
            onRefresh = { refreshKey++ },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        TabRow(selectedTabIndex = selectedTab) {
            Tab(selected = selectedTab == 0, onClick = { selectedTab = 0 },
                text = { Text("Диагностика") })
            Tab(selected = selectedTab == 1, onClick = { selectedTab = 1 },
                text = { Text("Логи") })
        }

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
            else -> when (selectedTab) {
                0 -> DiagnosticsTab(
                    info = info!!,
                    onOpenPath = { path -> scope.launch { root.diagnostics.openPath(path) } },
                )
                1 -> LogsScreen(logs = logs)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DiagnosticsToolbar(onBack: () -> Unit, onRefresh: () -> Unit) {
    TopAppBar(
        title = { Text("Диагностика", style = MaterialTheme.typography.titleMedium) },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
        },
        actions = {
            IconButton(onClick = onRefresh) {
                Icon(Icons.Default.Refresh, contentDescription = "Обновить")
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
    )
}

@Composable
private fun DiagnosticsTab(info: DiagnosticsInfo, onOpenPath: (String) -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 24.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        DiagSectionTitle("Система")
        DiagRow("ОС", info.os)
        DiagRow("Архитектура", info.arch)
        DiagRow("Версия", info.appVersion)
        DiagRow("CPAL Host", info.cpalHost)
        DiagStatusRow("FFmpeg", info.ffmpegOk, okLabel = "OK", failLabel = "Не найден")
        DiagStatusRow("API Key", info.hasAnthropicKey, okLabel = "Установлен", failLabel = "Отсутствует")

        Spacer(Modifier.height(8.dp))
        DiagSectionTitle("Аудио — вход")
        DeviceList(info.inputDevices)

        Spacer(Modifier.height(8.dp))
        DiagSectionTitle("Аудио — выход")
        DeviceList(info.outputDevices)

        Spacer(Modifier.height(8.dp))
        DiagSectionTitle("Пути")
        PathRow("Модель", info.paths.model, onOpenPath)
        PathRow("База данных", info.paths.db, onOpenPath)
        PathRow("Встречи", info.paths.meetingsDir, onOpenPath)
        PathRow("Промпты", info.paths.prompts, onOpenPath)
    }
}

@Composable
private fun DeviceList(devices: List<DeviceInfo>) {
    if (devices.isEmpty()) {
        Text(
            "Нет устройств",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    } else {
        devices.forEach { d ->
            val label = if (d.isDefault) "${d.name}  (по умолч.)" else d.name
            Text(
                "• $label",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun PathRow(label: String, info: PathInfo, onOpen: (String) -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                label,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.width(100.dp),
            )
            Text(
                info.path,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f),
            )
            IconButton(
                onClick = { onOpen(info.path) },
                modifier = Modifier.size(32.dp),
            ) {
                Icon(
                    Icons.Default.FolderOpen,
                    contentDescription = "Открыть",
                    modifier = Modifier.size(16.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Row(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.padding(start = 100.dp),
        ) {
            val existsColor = if (info.exists) MaterialTheme.colorScheme.primary
                              else MaterialTheme.colorScheme.error
            Text(
                if (info.exists) "✓ Существует" else "✗ Не найден",
                style = MaterialTheme.typography.labelSmall,
                color = existsColor,
            )
            if (info.exists && !info.writable) {
                Text(
                    "🔒 Только чтение",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.tertiary,
                )
            }
            info.sizeBytes?.let { size ->
                Text(
                    formatSize(size),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun DiagSectionTitle(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(bottom = 2.dp),
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

@Composable
private fun DiagStatusRow(label: String, ok: Boolean, okLabel: String, failLabel: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            if (ok) "✓ $okLabel" else "✗ $failLabel",
            style = MaterialTheme.typography.bodySmall,
            color = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
        )
    }
}

private fun formatSize(bytes: ULong): String = when {
    bytes < 1_024uL -> "$bytes B"
    bytes < 1_048_576uL -> "${"%.1f".format(bytes.toDouble() / 1_024.0)} KB"
    bytes < 1_073_741_824uL -> "${"%.1f".format(bytes.toDouble() / 1_048_576.0)} MB"
    else -> "${"%.2f".format(bytes.toDouble() / 1_073_741_824.0)} GB"
}
