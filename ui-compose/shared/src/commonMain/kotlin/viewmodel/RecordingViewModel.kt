package viewmodel

import com.arkivanov.essenty.instancekeeper.InstanceKeeper
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import repository.RecordingHandle
import repository.RecordingRepository

sealed interface RecordingUiState {
    data object Idle : RecordingUiState
    data class Recording(val id: String, val name: String, val elapsedSeconds: Int) : RecordingUiState
    data object Stopping : RecordingUiState
    data class Done(val handle: RecordingHandle) : RecordingUiState
    data class Error(val message: String) : RecordingUiState
}

class RecordingViewModel(
    private val repository: RecordingRepository,
) : InstanceKeeper.Instance {

    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    private val _state = MutableStateFlow<RecordingUiState>(RecordingUiState.Idle)
    val state: StateFlow<RecordingUiState> = _state.asStateFlow()

    private var timerJob: Job? = null

    fun start(name: String, source: String, echoCancel: Boolean) {
        if (_state.value !is RecordingUiState.Idle) return
        scope.launch {
            try {
                val handle = repository.start(name.ifBlank { "Встреча" }, source, echoCancel)
                _state.value = RecordingUiState.Recording(handle.id, handle.name, 0)
                launchTimer(handle.id, handle.name)
            } catch (e: Exception) {
                _state.value = RecordingUiState.Error(e.message ?: "Ошибка старта записи")
            }
        }
    }

    fun stop() {
        val current = _state.value as? RecordingUiState.Recording ?: return
        timerJob?.cancel()
        timerJob = null
        _state.value = RecordingUiState.Stopping
        scope.launch {
            try {
                val handle = repository.stop(current.id)
                _state.value = RecordingUiState.Done(handle)
            } catch (e: Exception) {
                _state.value = RecordingUiState.Error(e.message ?: "Ошибка остановки записи")
            }
        }
    }

    fun reset() {
        _state.value = RecordingUiState.Idle
    }

    private fun launchTimer(id: String, name: String) {
        timerJob = scope.launch {
            var elapsed = 0
            while (true) {
                delay(1000)
                elapsed++
                _state.value = RecordingUiState.Recording(id, name, elapsed)
            }
        }
    }

    override fun onDestroy() {
        timerJob?.cancel()
        scope.cancel()
    }
}
