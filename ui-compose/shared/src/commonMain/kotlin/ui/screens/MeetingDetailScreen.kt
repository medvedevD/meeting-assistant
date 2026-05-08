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
import com.mikepenz.markdown.m3.Markdown
import domain.Meeting
import domain.Protocol
import kotlinx.coroutines.launch
import ui.navigation.RootComponent
import ui.util.formatDate

@Composable
fun MeetingDetailScreen(meetingId: String, root: RootComponent) {
    var meeting by remember { mutableStateOf<Meeting?>(null) }
    var protocol by remember { mutableStateOf<Protocol?>(null) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    fun reload() { loading = true }

    LaunchedEffect(meetingId, loading) {
        if (!loading) return@LaunchedEffect
        error = null
        try {
            val all = root.meetings.list()
            meeting = all.firstOrNull { it.id == meetingId }
            protocol = root.meetings.protocolLoad(meetingId)
        } catch (e: Exception) {
            error = e.message
        } finally {
            loading = false
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        DetailToolbar(
            title = meeting?.name ?: "Встреча",
            showGenerate = !loading && error == null,
            onBack = { root.onBackToList() },
            onRefresh = { reload() },
            onGenerate = { root.onGenerateProtocol(meetingId) },
            onOpenFolder = meeting?.audioPath?.takeIf { it.isNotBlank() }?.let { path ->
                { scope.launch { root.diagnostics.openPath(parentDir(path)) } }
            },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        when {
            loading -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) { CircularProgressIndicator() }

            error != null -> ErrorPane(error!!)

            else -> {
                meeting?.let { MeetingMetaHeader(it) }
                HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)
                if (protocol != null) {
                    ProtocolPane(protocol!!.markdown)
                } else {
                    NoProtocolPane(
                        hasAudio = meeting?.audioPath?.isNotEmpty() == true,
                        onGenerate = { root.onGenerateProtocol(meetingId) },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DetailToolbar(
    title: String,
    showGenerate: Boolean,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    onGenerate: () -> Unit,
    onOpenFolder: (() -> Unit)?,
) {
    TopAppBar(
        title = { Text(title, style = MaterialTheme.typography.titleMedium) },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
        },
        actions = {
            if (showGenerate) {
                TextButton(onClick = onGenerate) { Text("Генерировать") }
            }
            if (onOpenFolder != null) {
                IconButton(onClick = onOpenFolder) {
                    Icon(Icons.Default.FolderOpen, contentDescription = "Открыть папку")
                }
            }
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
private fun MeetingMetaHeader(meeting: Meeting) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            formatDate(meeting.createdAt),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        if (meeting.hasTranscript) StatusBadge("Транскрипт")
        if (meeting.hasProtocol) StatusBadge("Протокол")
    }
}

@Composable
private fun StatusBadge(label: String) {
    Surface(
        color = MaterialTheme.colorScheme.primaryContainer,
        shape = MaterialTheme.shapes.small,
    ) {
        Text(
            label,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onPrimaryContainer,
        )
    }
}

@Composable
private fun ProtocolPane(markdown: String) {
    val scroll = rememberScrollState()
    Box(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
        Markdown(
            content = markdown,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 20.dp),
        )
    }
}

@Composable
private fun NoProtocolPane(hasAudio: Boolean, onGenerate: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                "Протокол ещё не сгенерирован",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (hasAudio) {
                Button(onClick = onGenerate) {
                    Text("Сгенерировать протокол")
                }
            } else {
                Text(
                    "Запишите встречу, чтобы сгенерировать протокол",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun ErrorPane(message: String) {
    Box(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            "Ошибка: $message",
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

private fun parentDir(path: String): String {
    val last = path.lastIndexOf('/')
    return if (last > 0) path.substring(0, last) else path
}
