import logging
import os
from pathlib import Path
import sys

_ROOT_DIR = Path(__file__).parent.parent.parent  # project root
_SCRIPTS_DIR = _ROOT_DIR / "scripts"
_log = logging.getLogger(__name__)


def _resolve_config_path() -> Path:
    if env := os.environ.get("MEETING_ASSISTANT_CONFIG"):
        return Path(env).expanduser()
    xdg = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(xdg) / "meeting-assistant" / "config.json"


def _default_meetings_dir() -> str:
    import subprocess
    try:
        docs = subprocess.check_output(
            ["xdg-user-dir", "DOCUMENTS"], text=True, timeout=2
        ).strip()
        if docs:
            return str(Path(docs) / "meetings")
    except Exception:
        pass
    return str(Path.home() / "Documents" / "meetings")

# scripts/ must be importable for gui_config (used by routes/config.py)
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from ..adapters.config.json_config import JsonConfigProvider
from ..adapters.config.filesystem_template_repo import FilesystemTemplateRepository
from ..adapters.storage.filesystem_meeting_repo import FilesystemMeetingRepository
from ..adapters.storage.sqlite_meeting_repo import SqliteMeetingRepository
from ..adapters.storage.sqlite_index import SqliteIndexAdapter
from ..adapters.storage.sqlite_job_repo import SqliteJobRepo
from ..adapters.transcription.faster_whisper import FasterWhisperTranscriber
from ..adapters.llm.provider_adapter import LLMProviderAdapter
from ..adapters.event_bus.in_process import InProcessEventBus
from ..application.process_meeting import ProcessMeetingUseCase
from ..application.regenerate_protocol import RegenerateProtocolUseCase
from ..application.list_meetings import ListMeetingsUseCase
from ..application.delete_meeting import DeleteMeetingUseCase
from ..application.manage_templates import ManageTemplatesUseCase
from ..application.events import TranscriptReady, ProtocolGenerated, MeetingDeleted


class Container:
    def __init__(self, root_dir: Path = _ROOT_DIR):
        self.root_dir = root_dir
        self.scripts_dir = root_dir / "scripts"
        self.config = JsonConfigProvider(_resolve_config_path())
        self.template_repo = FilesystemTemplateRepository(root_dir / "prompts")
        _fs_repo = FilesystemMeetingRepository(self._resolve_meetings_dir())
        self.meeting_repo = SqliteMeetingRepository(_fs_repo)
        self.meeting_repo.backfill()
        self.transcriber = FasterWhisperTranscriber(self.config)
        self.llm_provider = LLMProviderAdapter(self.config)

        self.event_bus = InProcessEventBus()
        self.search_index = SqliteIndexAdapter()

        self.event_bus.subscribe(TranscriptReady, self.search_index.on_transcript_ready)
        self.event_bus.subscribe(ProtocolGenerated, self.search_index.on_protocol_generated)
        self.event_bus.subscribe(MeetingDeleted, self.search_index.on_meeting_deleted)

        self.process_meeting = ProcessMeetingUseCase(
            transcriber=self.transcriber,
            llm_provider=self.llm_provider,
            meeting_repo=self.meeting_repo,
            template_repo=self.template_repo,
            config=self.config,
            event_bus=self.event_bus,
        )
        self.regenerate_protocol = RegenerateProtocolUseCase(
            llm_provider=self.llm_provider,
            meeting_repo=self.meeting_repo,
            template_repo=self.template_repo,
            config=self.config,
            event_bus=self.event_bus,
        )
        self.list_meetings = ListMeetingsUseCase(self.meeting_repo)
        self.delete_meeting = DeleteMeetingUseCase(self.meeting_repo, event_bus=self.event_bus)
        self.manage_templates = ManageTemplatesUseCase(self.template_repo)

        # ── RQ pipeline (optional, enabled by JOB_RUNNER=rq) ─────────────────
        self.job_runner = None
        self.job_repo = None
        self.pipeline_coordinator = None
        if os.environ.get("JOB_RUNNER", "").lower() == "rq":
            self._init_rq()


    def _init_rq(self) -> None:
        from ..adapters.jobs.rq_runner import RQRunner
        from ..application.orchestration.pipeline_coordinator import PipelineCoordinator
        redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
        self.job_runner = RQRunner(redis_url)
        if not self.job_runner.ping():
            _log.warning(
                "Redis not reachable at %s — RQ pipeline disabled. "
                "Start Redis or set JOB_RUNNER=legacy to suppress this warning.",
                redis_url,
            )
            self.job_runner = None
            return
        self.job_repo = SqliteJobRepo(self.meeting_repo._db_path)
        self.pipeline_coordinator = PipelineCoordinator(
            meeting_repo=self.meeting_repo,
            job_repo=self.job_repo,
            job_runner=self.job_runner,
        )
        _log.info("RQ pipeline enabled (Redis: %s)", redis_url)

    def _resolve_meetings_dir(self) -> Path:
        raw = (
            os.environ.get("MEETING_ASSISTANT_DATA_DIR")
            or self.config.get("storage", "meetings_dir", "")
            or _default_meetings_dir()
        )
        path = Path(raw).expanduser()
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        return path

    def rebuild_meeting_repo(self, new_path: str) -> None:
        path = Path(new_path).expanduser()
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        _fs_repo = FilesystemMeetingRepository(path)
        self.meeting_repo = SqliteMeetingRepository(_fs_repo)
        self.meeting_repo.backfill()
        self.process_meeting.meeting_repo = self.meeting_repo
        self.regenerate_protocol.meeting_repo = self.meeting_repo
        self.list_meetings.meeting_repo = self.meeting_repo
        self.delete_meeting.meeting_repo = self.meeting_repo
        if self.pipeline_coordinator is not None:
            self.pipeline_coordinator._repo = self.meeting_repo


def build_container(root_dir: Path = _ROOT_DIR) -> Container:
    return Container(root_dir)
