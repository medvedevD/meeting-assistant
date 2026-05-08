package ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private enum class LogLevel(val label: String, val marker: String) {
    ALL("Все", ""),
    ERROR("ERROR", "ERROR"),
    WARN("WARN", " WARN"),
    INFO("INFO", " INFO"),
    DEBUG("DEBUG", "DEBUG"),
    TRACE("TRACE", "TRACE"),
}

@Composable
fun LogsScreen(logs: List<String>) {
    var selectedLevel by remember { mutableStateOf(LogLevel.ALL) }

    val filtered = remember(logs, selectedLevel) {
        if (selectedLevel == LogLevel.ALL) logs
        else logs.filter { it.contains(selectedLevel.marker) }
    }

    val listState = rememberLazyListState()
    LaunchedEffect(filtered.size) {
        if (filtered.isNotEmpty()) listState.scrollToItem(filtered.lastIndex)
    }

    Column(modifier = Modifier.fillMaxSize()) {
        LevelFilterRow(selectedLevel = selectedLevel, onSelect = { selectedLevel = it })
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        if (filtered.isEmpty()) {
            Box(
                modifier = Modifier.fillMaxSize().padding(24.dp),
            ) {
                Text(
                    if (logs.isEmpty()) "Логи пусты" else "Нет строк с уровнем ${selectedLevel.label}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.3f)),
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(1.dp),
            ) {
                itemsIndexed(filtered, key = { i, line -> "$i:${line.take(60)}" }) { _, line ->
                    LogLine(line)
                }
            }
        }
    }
}

@Composable
private fun LevelFilterRow(selectedLevel: LogLevel, onSelect: (LogLevel) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        LogLevel.entries.forEach { level ->
            FilterChip(
                selected = selectedLevel == level,
                onClick = { onSelect(level) },
                label = { Text(level.label, style = MaterialTheme.typography.labelSmall) },
            )
        }
    }
}

@Composable
private fun LogLine(line: String) {
    val level = detectLevel(line)
    val color = levelColor(level)
    Text(
        text = line,
        style = MaterialTheme.typography.bodySmall.copy(
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
        ),
        color = color,
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 1.dp),
        softWrap = false,
    )
}

private fun detectLevel(line: String): LogLevel = when {
    line.contains("ERROR") -> LogLevel.ERROR
    line.contains(" WARN") -> LogLevel.WARN
    line.contains(" INFO") -> LogLevel.INFO
    line.contains("DEBUG") -> LogLevel.DEBUG
    line.contains("TRACE") -> LogLevel.TRACE
    else -> LogLevel.ALL
}

@Composable
private fun levelColor(level: LogLevel): Color = when (level) {
    LogLevel.ERROR -> MaterialTheme.colorScheme.error
    LogLevel.WARN  -> MaterialTheme.colorScheme.tertiary
    LogLevel.INFO  -> MaterialTheme.colorScheme.onSurface
    LogLevel.DEBUG -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.6f)
    LogLevel.TRACE -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.4f)
    LogLevel.ALL   -> MaterialTheme.colorScheme.onSurface
}
