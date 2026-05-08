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
import domain.Meeting
import domain.Protocol
import ui.navigation.RootComponent

@Composable
fun MeetingDetailScreen(meetingId: String, root: RootComponent) {
    var meeting by remember { mutableStateOf<Meeting?>(null) }
    var protocol by remember { mutableStateOf<Protocol?>(null) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(meetingId) {
        loading = true
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
            onBack = { root.onBackToList() },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.surfaceVariant)

        when {
            loading -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) { CircularProgressIndicator() }

            error != null -> ErrorPane(error!!)

            protocol != null -> ProtocolPane(protocol!!.markdown)

            else -> Box(
                modifier = Modifier.fillMaxSize().padding(32.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    "Протокол ещё не сгенерирован",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DetailToolbar(title: String, onBack: () -> Unit) {
    TopAppBar(
        title = { Text(title, style = MaterialTheme.typography.titleMedium) },
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
private fun ProtocolPane(markdown: String) {
    val scroll = rememberScrollState()
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp)
            .verticalScroll(scroll),
    ) {
        Text(
            text = markdown,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun ErrorPane(message: String) {
    Box(modifier = Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Text(
            "Ошибка: $message",
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}
