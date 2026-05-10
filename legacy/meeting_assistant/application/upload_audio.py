from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO

from ..domain.ports.meeting_repository import IMeetingRepository


@dataclass
class UploadResult:
    audio_path: Path
    slug: str
    folder: str


class UploadAudioUseCase:
    def __init__(self, repo: IMeetingRepository) -> None:
        self._repo = repo

    def execute(
        self,
        title: str,
        upload_stream: BinaryIO,
        original_filename: str,
    ) -> UploadResult:
        audio_path = self._repo.create_meeting_from_upload(title, upload_stream, original_filename)
        slug = audio_path.parent.name
        return UploadResult(audio_path=audio_path, slug=slug, folder=slug)
