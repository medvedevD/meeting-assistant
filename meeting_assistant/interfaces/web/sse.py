import json
from dataclasses import dataclass, asdict
from typing import Literal


@dataclass
class ProcessingEvent:
    stage: Literal["upload", "save", "transcribe", "protocol", "processing", "done", "error"]
    message: str = ""
    progress: float | None = None

    def to_sse(self) -> str:
        if self.stage == "done":
            return "event: done\ndata: {}\n\n"
        return f"data: {json.dumps(asdict(self), ensure_ascii=False)}\n\n"
