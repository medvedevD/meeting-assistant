from dataclasses import dataclass
from datetime import datetime


@dataclass
class Job:
    id: str
    meeting_slug: str
    kind: str                        # "transcribe" | "protocol"
    status: str                      # "queued" | "running" | "succeeded" | "failed" | "cancelled"
    attempt: int = 0
    max_attempts: int = 3
    progress_stage: str | None = None
    progress_value: float | None = None
    progress_eta_sec: float | None = None
    params_json: str | None = None
    error_message: str | None = None
    error_kind: str | None = None    # see error classification in plan
    rq_job_id: str | None = None
    enqueued_at: datetime | None = None
    started_at: datetime | None = None
    finished_at: datetime | None = None
    last_heartbeat_at: datetime | None = None
