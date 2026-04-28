from dataclasses import dataclass, field
from datetime import datetime
from ..value_objects.meeting_slug import MeetingSlug


@dataclass
class Meeting:
    slug: MeetingSlug
    title: str
    started_at: datetime | None = None
    duration_sec: float | None = None
    participants: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    has_transcript: bool = False
    has_protocol: bool = False
