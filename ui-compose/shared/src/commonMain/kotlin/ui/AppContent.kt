package ui

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.arkivanov.decompose.extensions.compose.subscribeAsState
import ui.navigation.RootComponent
import ui.navigation.Screen
import ui.screens.DiagnosticsScreen
import ui.screens.MeetingDetailScreen
import ui.screens.MeetingListScreen
import ui.screens.NewRecordingScreen
import ui.screens.SettingsScreen
import ui.theme.AppTheme

@Composable
fun AppContent(root: RootComponent) {
    AppTheme {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background,
        ) {
            Row(modifier = Modifier.fillMaxSize()) {
                Sidebar(
                    root = root,
                    modifier = Modifier.width(280.dp).fillMaxHeight(),
                )
                VerticalDivider(color = MaterialTheme.colorScheme.surfaceVariant)
                ContentPane(
                    root = root,
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                )
            }
        }
    }
}

@Composable
private fun ContentPane(root: RootComponent, modifier: Modifier = Modifier) {
    val screen by root.screen.subscribeAsState()
    Surface(modifier = modifier, color = MaterialTheme.colorScheme.surface) {
        when (val s = screen) {
            is Screen.MeetingList -> MeetingListScreen(root = root)
            is Screen.MeetingDetail -> MeetingDetailScreen(meetingId = s.meetingId, root = root)
            is Screen.NewRecording -> NewRecordingScreen(root = root)
            is Screen.Settings -> SettingsScreen(root = root)
            is Screen.Diagnostics -> DiagnosticsScreen(root = root)
        }
    }
}
