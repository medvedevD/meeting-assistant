from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from ..value_objects.meeting_slug import MeetingSlug
from ..value_objects.meeting_status import MeetingStatus


@dataclass
class Meeting:
    slug: MeetingSlug
    title: str
    status: MeetingStatus = MeetingStatus.CREATED
    started_at: datetime | None = None
    duration_sec: float | None = None
    audio_path: Path | None = None
    transcript_path: Path | None = None
    protocol_path: Path | None = None
    transcription_model: str | None = None
    protocol_model: str | None = None
    template_name: str | None = None
    participants: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    has_transcript: bool = False
    has_protocol: bool = False
    archived_at: datetime | None = None
