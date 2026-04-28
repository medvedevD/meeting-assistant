import logging
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

_log = logging.getLogger(__name__)

from ..domain.ports.llm_provider import ILLMProvider
from ..domain.ports.meeting_repository import IMeetingRepository
from ..domain.ports.template_repository import ITemplateRepository
from ..domain.ports.config_provider import IConfigProvider
from ..domain.ports.event_bus import IEventBus
from ..domain.value_objects.meeting_slug import MeetingSlug
from .events import ProtocolGenerated


@dataclass
class RegenerateProtocolRequest:
    slug: MeetingSlug
    meeting_name: str


@dataclass
class RegenerateProtocolResult:
    protocol_path: Path


class RegenerateProtocolUseCase:
    def __init__(
        self,
        llm_provider: ILLMProvider,
        meeting_repo: IMeetingRepository,
        template_repo: ITemplateRepository,
        config: IConfigProvider,
        event_bus: IEventBus | None = None,
    ):
        self._llm = llm_provider
        self._repo = meeting_repo
        self._templates = template_repo
        self._config = config
        self._bus = event_bus

    def execute(self, req: RegenerateProtocolRequest) -> RegenerateProtocolResult:
        date_str = datetime.now().strftime("%d.%m.%Y")
        transcript_text = self._repo.load_transcript_text(req.slug)
        instructions = self._build_instructions(transcript_text, req.meeting_name)
        meeting_dir = self._repo.get_meeting_dir(req.slug)
        partial_save_path = meeting_dir / "protocol.md"
        content = self._llm.generate(transcript_text, instructions, partial_save_path=partial_save_path)
        protocol_path = self._repo.save_protocol(req.slug, req.meeting_name, content, date_str)
        if self._bus:
            self._bus.publish(ProtocolGenerated(slug=req.slug, protocol_path=str(protocol_path), model=""))
        return RegenerateProtocolResult(protocol_path=protocol_path)

    def _build_instructions(self, transcript: str, meeting_name: str) -> str | None:
        active_name = self._config.get("protocol", "active_template", "")
        templates = self._templates.list()
        tmpl = next((t for t in templates if t["name"] == active_name), None)
        if not tmpl:
            return None
        pre, _, post = tmpl["prompt"].partition("{transcript}")
        return (pre + post).replace("{meeting_name}", meeting_name)
