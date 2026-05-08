package viewmodel

import com.arkivanov.essenty.instancekeeper.InstanceKeeper
import domain.Meeting
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import repository.MeetingRepository

sealed interface MeetingListState {
    data object Loading : MeetingListState
    data class Success(val meetings: List<Meeting>) : MeetingListState
    data class Error(val message: String) : MeetingListState
}

class MeetingListViewModel(
    private val repository: MeetingRepository,
) : InstanceKeeper.Instance {

    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    private val _state = MutableStateFlow<MeetingListState>(MeetingListState.Loading)
    val state: StateFlow<MeetingListState> = _state.asStateFlow()

    init {
        load()
    }

    fun refresh() = load()

    private fun load() {
        scope.launch {
            _state.value = MeetingListState.Loading
            try {
                _state.value = MeetingListState.Success(repository.list())
            } catch (e: Exception) {
                _state.value = MeetingListState.Error(e.message ?: "Unknown error")
            }
        }
    }

    override fun onDestroy() {
        scope.cancel()
    }
}
