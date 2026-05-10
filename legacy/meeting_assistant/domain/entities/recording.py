from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Recording:
    path: Path
    format: str = "wav"
    sample_rate: int = 44100
    channels: int = 2
