from abc import ABC, abstractmethod
from collections.abc import Callable
from ..entities.recording import Recording
from ..entities.transcript import Transcript


class ITranscriber(ABC):
    @abstractmethod
    def transcribe(
        self,
        recording: Recording,
        model_name: str,
        on_progress: Callable[[float], None] | None = None,
    ) -> Transcript: ...
