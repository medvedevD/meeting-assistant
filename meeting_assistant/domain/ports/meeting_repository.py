from abc import ABC, abstractmethod
from pathlib import Path
from typing import BinaryIO
from ..entities.meeting import Meeting
from ..value_objects.meeting_slug import MeetingSlug


class IMeetingRepository(ABC):
    @abstractmethod
    def resolve_slug_for_audio(self, audio_path: Path, meeting_name: str) -> MeetingSlug: ...

    @abstractmethod
    def create_meeting_from_upload(
        self,
        title: str,
        upload_stream: BinaryIO,
        original_filename: str,
    ) -> Path: ...

    @abstractmethod
    def get_meeting_dir(self, slug: MeetingSlug) -> Path: ...

    @abstractmethod
    def save_transcript(self, slug: MeetingSlug, meeting_name: str, text: str, date_str: str) -> Path: ...

    @abstractmethod
    def load_transcript_text(self, slug: MeetingSlug) -> str: ...

    @abstractmethod
    def save_protocol(self, slug: MeetingSlug, meeting_name: str, content: str, date_str: str) -> Path: ...

    @abstractmethod
    def list(self) -> list[Meeting]: ...

    @abstractmethod
    def delete(self, slug: MeetingSlug) -> None: ...
