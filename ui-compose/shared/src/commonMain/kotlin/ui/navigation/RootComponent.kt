package ui.navigation

import com.arkivanov.decompose.ComponentContext
import com.arkivanov.decompose.value.MutableValue
import com.arkivanov.decompose.value.Value
import com.arkivanov.essenty.instancekeeper.getOrCreate
import repository.DiagnosticsRepository
import repository.MeetingRepository
import repository.RecordingRepository
import repository.SettingsRepository
import viewmodel.MeetingListViewModel
import viewmodel.RecordingViewModel

sealed interface Screen {
    data object MeetingList : Screen
    data class MeetingDetail(val meetingId: String) : Screen
    data class GenerateProtocol(val meetingId: String) : Screen
    data object NewRecording : Screen
    data object Settings : Screen
    data object Diagnostics : Screen
}

class RootComponent(
    componentContext: ComponentContext,
    val meetings: MeetingRepository,
    val recording: RecordingRepository,
    val settings: SettingsRepository,
    val diagnostics: DiagnosticsRepository,
) : ComponentContext by componentContext {

    private val _screen = MutableValue<Screen>(Screen.MeetingList)
    val screen: Value<Screen> get() = _screen

    // Survive recomposition; onDestroy() cancels their coroutine scopes.
    val meetingListViewModel: MeetingListViewModel = instanceKeeper.getOrCreate("meeting-list") {
        MeetingListViewModel(meetings)
    }
    val recordingViewModel: RecordingViewModel = instanceKeeper.getOrCreate("recording") {
        RecordingViewModel(recording)
    }

    fun navigate(screen: Screen) {
        _screen.value = screen
    }

    fun onMeetingSelected(id: String) = navigate(Screen.MeetingDetail(id))
    fun onGenerateProtocol(meetingId: String) = navigate(Screen.GenerateProtocol(meetingId))
    fun onNewRecordingRequested() = navigate(Screen.NewRecording)
    fun onSettingsRequested() = navigate(Screen.Settings)
    fun onDiagnosticsRequested() = navigate(Screen.Diagnostics)
    fun onBackToList() = navigate(Screen.MeetingList)
}
