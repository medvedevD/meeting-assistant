package viewmodel

import domain.JobStatus
import domain.Protocol
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import repository.MeetingRepository

sealed interface GenerateState {
    data object Idle : GenerateState
    data class Transcribing(val jobId: String, val attempts: Int) : GenerateState
    data class TranscriptFailed(val error: String) : GenerateState
    data object GeneratingProtocol : GenerateState
    data class Done(val protocol: Protocol) : GenerateState
    data class Failed(val error: String) : GenerateState
}

class ProtocolGenerateViewModel(
    private val repository: MeetingRepository,
) {
    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    private val _state = MutableStateFlow<GenerateState>(GenerateState.Idle)
    val state: StateFlow<GenerateState> = _state.asStateFlow()

    fun generate(
        meetingId: String,
        audioPath: String,
        hasTranscript: Boolean,
        templateName: String?,
        meetingName: String?,
    ) {
        if (_state.value !is GenerateState.Idle
            && _state.value !is GenerateState.TranscriptFailed
            && _state.value !is GenerateState.Failed
        ) return

        scope.launch {
            if (!hasTranscript) {
                if (!submitAndPollTranscription(audioPath, meetingName)) return@launch
            }
            generateProtocol(meetingId, templateName)
        }
    }

    fun reset() {
        _state.value = GenerateState.Idle
    }

    private suspend fun submitAndPollTranscription(audioPath: String, name: String?): Boolean {
        val job = try {
            repository.jobSubmit(audioPath, name)
        } catch (e: Exception) {
            _state.value = GenerateState.Failed(e.message ?: "Не удалось запустить транскрипцию")
            return false
        }
        _state.value = GenerateState.Transcribing(job.id, 0)

        while (true) {
            delay(2_000)
            val status = try {
                repository.jobStatus(job.id)
            } catch (_: Exception) {
                continue // transient error — keep polling
            } ?: continue

            when (status.status) {
                JobStatus.DONE -> return true
                JobStatus.FAILED -> {
                    _state.value = GenerateState.TranscriptFailed(
                        status.lastError ?: "Транскрипция завершилась с ошибкой"
                    )
                    return false
                }
                else -> _state.value = GenerateState.Transcribing(job.id, status.attempts)
            }
        }
    }

    private suspend fun generateProtocol(meetingId: String, templateName: String?) {
        _state.value = GenerateState.GeneratingProtocol
        try {
            val protocol = repository.protocolGenerate(meetingId, templateName)
            _state.value = GenerateState.Done(protocol)
        } catch (e: Exception) {
            _state.value = GenerateState.Failed(e.message ?: "Ошибка генерации протокола")
        }
    }

    fun onDestroy() {
        scope.cancel()
    }
}
