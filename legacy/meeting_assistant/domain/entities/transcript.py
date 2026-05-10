from dataclasses import dataclass, field
from ..value_objects.transcript_segment import TranscriptSegment


@dataclass
class Transcript:
    segments: list[TranscriptSegment] = field(default_factory=list)
    language: str = "ru"
    language_probability: float = 1.0

    def to_text(self) -> str:
        lines = []
        for seg in self.segments:
            start = int(seg.start)
            m, s = divmod(start, 60)
            prefix = f"[{m:02d}:{s:02d}] "
            speaker = f"{seg.speaker}: " if seg.speaker else ""
            lines.append(prefix + speaker + seg.text)
        return "\n".join(lines)
