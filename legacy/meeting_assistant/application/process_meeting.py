import logging
from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from ..domain.ports.transcriber import ITranscriber
from ..domain.ports.llm_provider import ILLMProvider
from ..domain.ports.meeting_repository import IMeetingRepository
from ..domain.ports.template_repository import ITemplateRepository
from ..domain.ports.config_provider import IConfigProvider
from ..domain.ports.event_bus import IEventBus
from ..domain.entities.recording import Recording
from ..domain.value_objects.meeting_status import MeetingStatus
from .events import TranscriptReady, ProtocolGenerated

_log = logging.getLogger(__name__)


@dataclass
class ProcessMeetingRequest:
    audio_path: Path
    meeting_name: str
    whisper_model: str
    generate_protocol: bool = True
    from_transcript: bool = False


@dataclass
class ProcessMeetingResult:
    transcript_path: Path
    protocol_path: Path | None
    meeting_name: str


class ProcessMeetingUseCase:
    def __init__(
        self,
        transcriber: ITranscriber,
        llm_provider: ILLMProvider,
        meeting_repo: IMeetingRepository,
        template_repo: ITemplateRepository,
        config: IConfigProvider,
        event_bus: IEventBus | None = None,
    ):
        self._transcriber = transcriber
        self._llm = llm_provider
        self._repo = meeting_repo
        self._templates = template_repo
        self._config = config
        self._bus = event_bus

    def execute(
        self,
        req: ProcessMeetingRequest,
        on_stage: Callable[[str], None] | None = None,
        on_progress: Callable[[float], None] | None = None,
    ) -> ProcessMeetingResult:
        def notify(stage: str) -> None:
            if on_stage:
                on_stage(stage)

        date_str = datetime.now().strftime("%d.%m.%Y")
        slug = self._repo.resolve_slug_for_audio(req.audio_path, req.meeting_name)
        meeting_dir = self._repo.get_meeting_dir(slug)
        meeting_dir.mkdir(parents=True, exist_ok=True)

        notify("save")

        if req.from_transcript:
            _log.info("Читаю транскрипцию: %s", meeting_dir / "transcript.md")
            transcript_text = self._repo.load_transcript_text(slug)
            _log.info("Транскрипция загружена (%d строк)", len(transcript_text.splitlines()))
            transcript_path = meeting_dir / "transcript.md"
        else:
            notify("transcribe")
            _log.info("Транскрибирую: %s", req.audio_path)
            self._try_update_status(slug, MeetingStatus.TRANSCRIBING)
            recording = Recording(path=req.audio_path)
            try:
                transcript = self._transcriber.transcribe(recording, req.whisper_model, on_progress=on_progress)
            except Exception:
                self._try_update_status(slug, MeetingStatus.TRANSCRIBE_FAILED)
                raise
            transcript_text = transcript.to_text()
            transcript_path = self._repo.save_transcript(slug, req.meeting_name, transcript_text, date_str)
            _log.info("Транскрипция сохранена: %s", transcript_path)
            if self._bus:
                self._bus.publish(TranscriptReady(slug=slug, transcript_path=str(transcript_path)))

        protocol_path = None
        if req.generate_protocol:
            notify("protocol")
            protocol_path = meeting_dir / "protocol.md"
            self._try_update_status(slug, MeetingStatus.GENERATING)
            instructions = self._build_instructions(transcript_text, req.meeting_name)
            try:
                content = self._llm.generate(transcript_text, instructions, partial_save_path=protocol_path)
            except Exception:
                self._try_update_status(slug, MeetingStatus.PROTOCOL_FAILED)
                raise
            protocol_path = self._repo.save_protocol(slug, req.meeting_name, content, date_str)
            _log.info("Протокол сохранён: %s", protocol_path)
            if self._bus:
                self._bus.publish(ProtocolGenerated(slug=slug, protocol_path=str(protocol_path), model=""))

        return ProcessMeetingResult(
            transcript_path=transcript_path,
            protocol_path=protocol_path,
            meeting_name=req.meeting_name,
        )

    def _try_update_status(self, slug, status: MeetingStatus) -> None:
        try:
            self._repo.update_status(slug, status)
        except Exception:
            pass  # state machine violation or DB error — don't break the pipeline

    def _build_instructions(self, transcript: str, meeting_name: str) -> str | None:
        active_name = self._config.get("protocol", "active_template", "")
        templates = self._templates.list()
        tmpl = next((t for t in templates if t["name"] == active_name), None)
        if not tmpl:
            return None
        pre, _, post = tmpl["prompt"].partition("{transcript}")
        return (pre + post).replace("{meeting_name}", meeting_name)
