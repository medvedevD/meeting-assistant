from dataclasses import dataclass
from ..domain.value_objects.meeting_slug import MeetingSlug


@dataclass
class MeetingRecorded:
    slug: MeetingSlug


@dataclass
class TranscriptReady:
    slug: MeetingSlug
    transcript_path: str


@dataclass
class ProtocolGenerated:
    slug: MeetingSlug
    protocol_path: str
    model: str


@dataclass
class MeetingDeleted:
    slug: MeetingSlug
