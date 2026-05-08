package ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.arkivanov.decompose.extensions.compose.subscribeAsState
import domain.Meeting
import ui.navigation.RootComponent
import ui.navigation.Screen
import ui.util.formatDate
import viewmodel.MeetingListState

@Composable
fun Sidebar(root: RootComponent, modifier: Modifier = Modifier) {
    val screen by root.screen.subscribeAsState()
    val vmState by root.meetingListViewModel.state.collectAsState()

    Column(
        modifier = modifier.background(MaterialTheme.colorScheme.surfaceVariant),
    ) {
        SidebarHeader(
            onNewRecording = { root.onNewRecordingRequested() },
            onRefresh = { root.meetingListViewModel.refresh() },
        )
        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))

        when (val state = vmState) {
            is MeetingListState.Loading -> Box(
                modifier = Modifier.weight(1f).fillMaxWidth(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(modifier = Modifier.size(24.dp))
            }

            is MeetingListState.Error -> Box(
                modifier = Modifier.weight(1f).fillMaxWidth().padding(16.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    "Ошибка: ${state.message}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            is MeetingListState.Success -> {
                val meetings = state.meetings
                LazyColumn(modifier = Modifier.weight(1f)) {
                    items(meetings, key = { it.id }) { meeting ->
                        val selected = screen is Screen.MeetingDetail &&
                            (screen as Screen.MeetingDetail).meetingId == meeting.id
                        MeetingListItem(
                            meeting = meeting,
                            selected = selected,
                            onClick = { root.onMeetingSelected(meeting.id) },
                        )
                    }
                    if (meetings.isEmpty()) {
                        item {
                            Box(
                                Modifier.fillMaxWidth().padding(24.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    "Нет встреч",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
        }

        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f))
        SidebarFooter(
            onSettings = { root.onSettingsRequested() },
            onDiagnostics = { root.onDiagnosticsRequested() },
        )
    }
}

@Composable
private fun SidebarHeader(onNewRecording: () -> Unit, onRefresh: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            "Встречи",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = onNewRecording, modifier = Modifier.size(32.dp)) {
            Icon(
                Icons.Default.Add,
                contentDescription = "Новая запись",
                tint = MaterialTheme.colorScheme.primary,
            )
        }
    }
}

@Composable
private fun SidebarFooter(onSettings: () -> Unit, onDiagnostics: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(4.dp),
        horizontalArrangement = Arrangement.End,
    ) {
        IconButton(onClick = onDiagnostics) {
            Icon(
                Icons.Default.Build,
                contentDescription = "Диагностика",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        IconButton(onClick = onSettings) {
            Icon(
                Icons.Default.Settings,
                contentDescription = "Настройки",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun MeetingListItem(
    meeting: Meeting,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val bg = if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
             else MaterialTheme.colorScheme.surfaceVariant
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(bg)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                meeting.name,
                style = MaterialTheme.typography.bodyMedium,
                color = if (selected) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
            Text(
                formatDate(meeting.createdAt),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (meeting.hasTranscript) StatusChip("Транскрипт")
                if (meeting.hasProtocol) StatusChip("Протокол")
            }
        }
    }
}

@Composable
private fun StatusChip(label: String) {
    Text(
        label,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.primary,
    )
}
