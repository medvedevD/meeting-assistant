from abc import ABC, abstractmethod
from ..entities.recording import Recording
from ..entities.transcript import Transcript


class ITranscriber(ABC):
    @abstractmethod
    def transcribe(self, recording: Recording, model_name: str) -> Transcript: ...
