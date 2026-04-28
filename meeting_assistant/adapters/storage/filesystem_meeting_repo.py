import os
import re
import shutil
import tempfile
from datetime import datetime
from pathlib import Path
from typing import BinaryIO

from ...domain.ports.meeting_repository import IMeetingRepository
from ...domain.entities.meeting import Meeting
from ...domain.value_objects.meeting_slug import MeetingSlug

_UNSAFE = re.compile(r"[^\w\-]")

_ALLOWED_EXTENSIONS = {".wav", ".mp3", ".m4a", ".flac", ".ogg", ".opus", ".webm"}

# Magic-bytes signatures for supported audio formats
_MAGIC: list[tuple[bytes, str]] = [
    (b"RIFF", ".wav"),
    (b"ID3", ".mp3"),
    (b"\xff\xfb", ".mp3"),
    (b"\xff\xf3", ".mp3"),
    (b"\xff\xf2", ".mp3"),
    (b"fLaC", ".flac"),
    (b"OggS", ".ogg"),
    (b"\x1aE\xdf\xa3", ".webm"),  # EBML/WebM
]


def _detect_extension(header: bytes, original_filename: str) -> str:
    for magic, ext in _MAGIC:
        if header[: len(magic)] == magic:
            return ext
    # m4a/mp4 has "ftyp" at offset 4
    if len(header) >= 8 and header[4:8] == b"ftyp":
        return ".m4a"
    # Opus in Ogg container is already caught by OggS; bare Opus (.opus) is rare
    suffix = Path(original_filename).suffix.lower()
    if suffix in _ALLOWED_EXTENSIONS:
        return suffix
    return ".audio"


class FilesystemMeetingRepository(IMeetingRepository):
    def __init__(self, meetings_dir: Path):
        self._dir = meetings_dir.resolve()

    @property
    def meetings_dir(self) -> Path:
        return self._dir

    def create_meeting_from_upload(
        self,
        title: str,
        upload_stream: BinaryIO,
        original_filename: str,
    ) -> Path:
        date_prefix = datetime.now().strftime("%Y-%m-%d")
        if title.strip():
            safe = _UNSAFE.sub("_", title.strip())
            base_slug = f"{date_prefix}_{safe}"
        else:
            ts = datetime.now().strftime("%Y-%m-%d_%H%M%S")
            base_slug = f"upload-{ts}"

        # Ensure unique slug
        slug_str = base_slug
        counter = 2
        while (self._dir / slug_str).exists():
            slug_str = f"{base_slug}-{counter}"
            counter += 1

        meeting_dir = self._dir / slug_str
        meeting_dir.mkdir(parents=True, exist_ok=True)

        cache_dir = Path.home() / ".cache" / "meeting-assistant" / "uploads"
        cache_dir.mkdir(parents=True, exist_ok=True)

        # Stream to temp file and detect format from first bytes
        with tempfile.NamedTemporaryFile(dir=cache_dir, suffix=".part", delete=False) as tmp:
            tmp_path = Path(tmp.name)
            header = b""
            first = True
            for chunk in iter(lambda: upload_stream.read(65536), b""):
                if first:
                    header = chunk[:16]
                    first = False
                tmp.write(chunk)

        ext = _detect_extension(header, original_filename)
        dest = meeting_dir / f"recording{ext}"
        os.replace(tmp_path, dest)
        return dest

    def resolve_slug_for_audio(self, audio_path: Path, meeting_name: str) -> MeetingSlug:
        """Return the slug for a meeting. If audio is meetings/<slug>/recording.wav, reuse that slug."""
        audio_path = audio_path.resolve()
        if audio_path.name == "recording.wav" and audio_path.parent.parent == self._dir:
            return MeetingSlug(audio_path.parent.name)
        date_prefix = datetime.now().strftime("%Y-%m-%d")
        safe = _UNSAFE.sub("_", meeting_name)
        return MeetingSlug(f"{date_prefix}_{safe}")

    def get_meeting_dir(self, slug: MeetingSlug) -> Path:
        return self._dir / slug.value

    def save_transcript(self, slug: MeetingSlug, meeting_name: str, text: str, date_str: str) -> Path:
        path = self.get_meeting_dir(slug) / "transcript.md"
        path.write_text(
            f"# Транскрипция: {meeting_name}\n**Дата:** {date_str}\n\n{text}",
            encoding="utf-8",
        )
        return path

    def load_transcript_text(self, slug: MeetingSlug) -> str:
        """Read transcript.md and strip the Markdown header, returning raw transcript lines."""
        path = self.get_meeting_dir(slug) / "transcript.md"
        lines = path.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            stripped = line.strip()
            if stripped and not stripped.startswith("#") and not stripped.startswith("**"):
                return "\n".join(lines[i:])
        return ""

    def save_protocol(self, slug: MeetingSlug, meeting_name: str, content: str, date_str: str) -> Path:
        path = self.get_meeting_dir(slug) / "protocol.md"
        path.write_text(
            f"# Протокол встречи: {meeting_name}\n**Дата:** {date_str}\n\n{content}",
            encoding="utf-8",
        )
        return path

    def list(self) -> list[Meeting]:
        if not self._dir.exists():
            return []
        meetings = []
        for folder in sorted(self._dir.iterdir(), reverse=True):
            if not folder.is_dir():
                continue
            try:
                slug = MeetingSlug(folder.name)
            except ValueError:
                continue
            title = folder.name.replace("_", " ")
            started_at = _parse_date_from_slug(folder.name)
            meetings.append(Meeting(
                slug=slug,
                title=title,
                started_at=started_at,
                has_transcript=(folder / "transcript.md").exists(),
                has_protocol=(folder / "protocol.md").exists(),
            ))
        return meetings

    def delete(self, slug: MeetingSlug) -> None:
        meeting_dir = self.get_meeting_dir(slug)
        if meeting_dir.exists():
            shutil.rmtree(meeting_dir)


def _parse_date_from_slug(slug: str) -> datetime | None:
    """Try to extract a date from a slug like '2024-01-15_...'."""
    try:
        return datetime.strptime(slug[:10], "%Y-%m-%d")
    except (ValueError, IndexError):
        return None
