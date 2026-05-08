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

sealed interface Screen {
    data object MeetingList : Screen
    data class MeetingDetail(val meetingId: String) : Screen
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

    // Survives recomposition; onDestroy() cancels its coroutine scope.
    val meetingListViewModel: MeetingListViewModel = instanceKeeper.getOrCreate {
        MeetingListViewModel(meetings)
    }

    fun navigate(screen: Screen) {
        _screen.value = screen
    }

    fun onMeetingSelected(id: String) = navigate(Screen.MeetingDetail(id))
    fun onNewRecordingRequested() = navigate(Screen.NewRecording)
    fun onSettingsRequested() = navigate(Screen.Settings)
    fun onDiagnosticsRequested() = navigate(Screen.Diagnostics)
    fun onBackToList() = navigate(Screen.MeetingList)
}
