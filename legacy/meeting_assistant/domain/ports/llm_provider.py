from abc import ABC, abstractmethod
from pathlib import Path


class ILLMProvider(ABC):
    @abstractmethod
    def generate(self, transcript: str, instructions: str | None, partial_save_path: Path, *, model: str | None = None) -> str: ...
