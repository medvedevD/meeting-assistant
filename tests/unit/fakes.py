"""In-memory implementations of all domain ports for unit testing."""
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from meeting_assistant.domain.entities.meeting import Meeting
from meeting_assistant.domain.entities.recording import Recording
from meeting_assistant.domain.entities.transcript import Transcript
from meeting_assistant.domain.ports.config_provider import IConfigProvider
from meeting_assistant.domain.ports.event_bus import IEventBus
from meeting_assistant.domain.ports.llm_provider import ILLMProvider
from meeting_assistant.domain.ports.meeting_repository import IMeetingRepository
from meeting_assistant.domain.ports.template_repository import ITemplateRepository
from meeting_assistant.domain.ports.transcriber import ITranscriber
from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
from meeting_assistant.domain.value_objects.transcript_segment import TranscriptSegment


class FakeConfigProvider(IConfigProvider):
    def __init__(self, data: dict | None = None):
        self._data: dict = data or {}

    def get(self, section: str, key: str, default=None):
        return self._data.get(section, {}).get(key, default)

    def load(self) -> dict:
        return dict(self._data)

    def save(self, cfg: dict) -> None:
        self._data = cfg


class FakeTranscriber(ITranscriber):
    def __init__(self, result_text: str = "[00:00] Test transcript line"):
        self._text = result_text
        self.calls: list[tuple[Recording, str]] = []

    def transcribe(self, recording: Recording, model_name: str) -> Transcript:
        self.calls.append((recording, model_name))
        seg = TranscriptSegment(start=0.0, end=5.0, text=self._text)
        return Transcript(segments=[seg])


class FakeLLMProvider(ILLMProvider):
    def __init__(self, result: str = "Protocol content"):
        self._result = result
        self.calls: list[tuple[str, str | None]] = []

    def generate(self, transcript: str, instructions: str | None, partial_save_path: Path) -> str:
        self.calls.append((transcript, instructions))
        return self._result


@dataclass
class _MeetingRecord:
    slug: MeetingSlug
    transcript: str = ""
    protocol: str = ""
    deleted: bool = False


class FakeMeetingRepository(IMeetingRepository):
    def __init__(self, base_dir: Path | None = None):
        self._records: dict[str, _MeetingRecord] = {}
        self._base = base_dir or Path("/fake/meetings")

    def resolve_slug_for_audio(self, audio_path: Path, meeting_name: str) -> MeetingSlug:
        return MeetingSlug.from_title(meeting_name, "2024-01-01")

    def get_meeting_dir(self, slug: MeetingSlug) -> Path:
        return self._base / slug.value

    def save_transcript(self, slug: MeetingSlug, meeting_name: str, text: str, date_str: str) -> Path:
        rec = self._records.setdefault(slug.value, _MeetingRecord(slug=slug))
        rec.transcript = text
        return self._base / slug.value / "transcript.md"

    def load_transcript_text(self, slug: MeetingSlug) -> str:
        rec = self._records.get(slug.value)
        if not rec or not rec.transcript:
            raise FileNotFoundError(f"No transcript for {slug.value}")
        return rec.transcript

    def save_protocol(self, slug: MeetingSlug, meeting_name: str, content: str, date_str: str) -> Path:
        rec = self._records.setdefault(slug.value, _MeetingRecord(slug=slug))
        rec.protocol = content
        return self._base / slug.value / "protocol.md"

    def list(self) -> list[Meeting]:
        return [
            Meeting(slug=r.slug, title=r.slug.value.replace("_", " "))
            for r in self._records.values()
            if not r.deleted
        ]

    def delete(self, slug: MeetingSlug) -> None:
        rec = self._records.get(slug.value)
        if rec:
            rec.deleted = True

    def has_transcript(self, slug: MeetingSlug) -> bool:
        rec = self._records.get(slug.value)
        return bool(rec and rec.transcript)

    def has_protocol(self, slug: MeetingSlug) -> bool:
        rec = self._records.get(slug.value)
        return bool(rec and rec.protocol)


class FakeTemplateRepository(ITemplateRepository):
    def __init__(self, templates: list[dict] | None = None):
        self._templates: list[dict] = templates or []

    def list(self) -> list[dict]:
        return list(self._templates)

    def save(self, name: str, prompt: str) -> None:
        for t in self._templates:
            if t["name"] == name:
                t["prompt"] = prompt
                return
        self._templates.append({"name": name, "prompt": prompt})

    def delete(self, name: str) -> None:
        self._templates = [t for t in self._templates if t["name"] != name]


class FakeEventBus(IEventBus):
    def __init__(self):
        self.published: list[Any] = []
        self._handlers: dict[type, list[Callable]] = {}

    def publish(self, event: Any) -> None:
        self.published.append(event)
        for handler in self._handlers.get(type(event), []):
            handler(event)

    def subscribe(self, event_type: type, handler: Callable[[Any], None]) -> None:
        self._handlers.setdefault(event_type, []).append(handler)

    def events_of(self, event_type: type) -> list[Any]:
        return [e for e in self.published if isinstance(e, event_type)]
