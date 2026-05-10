"""
RQ worker function: transcription.

Called by RQ worker process as:
  rq worker transcribe
"""
import json
import logging
import os
from pathlib import Path

_log = logging.getLogger(__name__)


class _CancelledError(Exception):
    pass


def run_transcribe_job(job_id: str, slug: str, params: dict) -> None:
    """
    Transcribe a meeting's audio file.

    params keys:
      model        — Whisper model name (default from config)
      auto_protocol — whether to auto-enqueue protocol job after success (default True)
      template      — protocol template name (forwarded to protocol job)
    """
    from meeting_assistant.infrastructure.container import build_container
    from meeting_assistant.adapters.jobs.progress_reporter import ProgressReporter
    from meeting_assistant.adapters.jobs.error_classification import (
        classify_error, RETRYABLE_ERRORS, RETRY_BACKOFF,
    )
    from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo
    from meeting_assistant.domain.entities.recording import Recording
    from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
    from meeting_assistant.domain.value_objects.meeting_status import MeetingStatus

    redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
    container = build_container()
    db_path = container.meeting_repo._db_path
    job_repo = SqliteJobRepo(db_path)

    # Increment attempt counter before marking running so UI shows correct attempt.
    job_repo.increment_attempt(job_id)

    reporter = ProgressReporter(job_id=job_id, slug=slug, db_path=db_path, redis_url=redis_url)
    reporter.mark_running()

    meeting_slug = MeetingSlug(slug)
    meeting = container.meeting_repo.get(meeting_slug)
    if meeting is None:
        reporter.mark_failed(f"Meeting {slug} not found", "unknown")
        return

    audio_path = meeting.audio_path
    if audio_path is None:
        # Fall back to filesystem scan for legacy meetings
        meeting_dir = container.meeting_repo.get_meeting_dir(meeting_slug)
        recordings = list(meeting_dir.glob("recording.*"))
        if not recordings:
            reporter.mark_failed("No audio file found", "audio_corrupt")
            return
        audio_path = recordings[0]

    model = params.get("model") or container.config.get("transcription", "model", "medium")

    try:
        reporter.update("loading_model", None)

        if reporter.should_cancel():
            raise _CancelledError("Job cancelled by user")

        def on_progress(value: float) -> None:
            if reporter.should_cancel():
                raise _CancelledError("Job cancelled by user")
            reporter.update("transcribing", value)

        recording = Recording(path=audio_path)
        transcript = container.transcriber.transcribe(recording, model, on_progress=on_progress)

        reporter.update("saving", None)
        from datetime import datetime
        date_str = datetime.now().strftime("%d.%m.%Y")
        container.meeting_repo.save_transcript(
            meeting_slug, meeting.title, transcript.to_text(), date_str
        )

        # Persist the model used so the card can show it
        try:
            device = container.config.get("transcription", "device", "cpu")
            compute = container.config.get("transcription", "compute_type", "int8")
            container.meeting_repo.update_processing_metadata(
                meeting_slug,
                transcription_model=f"faster-whisper:{model}/{device}/{compute}",
            )
        except Exception:
            pass

        # Publish transcript-ready event to search index
        from meeting_assistant.application.events import TranscriptReady
        transcript_path = container.meeting_repo.get_meeting_dir(meeting_slug) / "transcript.md"
        container.event_bus.publish(TranscriptReady(slug=meeting_slug, transcript_path=str(transcript_path)))

        if params.get("auto_protocol", True):
            # Mid-pipeline: mark DB succeeded but keep SSE open for protocol
            reporter.mark_succeeded_no_publish()
            _publish_meeting_status(reporter, "TRANSCRIBED")
            reporter._publish({"type": "transcription_done"})
            _enqueue_protocol(slug, job_id, params, redis_url, db_path)
        else:
            reporter.mark_succeeded()
            _publish_meeting_status(reporter, "TRANSCRIBED")

    except _CancelledError:
        _log.info("Transcription cancelled for %s", slug)
        try:
            container.meeting_repo.update_status(meeting_slug, MeetingStatus.AUDIO_READY)
        except Exception:
            pass
        reporter.mark_cancelled()

    except Exception as exc:
        _log.exception("Transcription failed for %s", slug)
        error_kind = classify_error(exc)
        try:
            container.meeting_repo.update_status(meeting_slug, MeetingStatus.TRANSCRIBE_FAILED)
        except Exception:
            pass
        reporter.mark_failed(str(exc), error_kind)

        if error_kind in RETRYABLE_ERRORS:
            job = job_repo.get(job_id)
            if job and job.attempt < job.max_attempts:
                delay = RETRY_BACKOFF[min(job.attempt - 1, len(RETRY_BACKOFF) - 1)]
                _log.info(
                    "Auto-retry transcribe for %s in %ds (attempt %d/%d)",
                    slug, delay, job.attempt, job.max_attempts,
                )
                _schedule_retry(slug, job_id, params, redis_url, db_path, delay, "transcribe")
                return  # handled — don't re-raise
        raise


def _publish_meeting_status(reporter: "ProgressReporter", status: str) -> None:
    reporter._publish({"type": "meeting_status", "status": status})


def _schedule_retry(
    slug: str,
    job_id: str,
    params: dict,
    redis_url: str,
    db_path: Path,
    delay_sec: int,
    kind: str,
) -> None:
    """Re-enqueue the same job after delay_sec seconds."""
    try:
        import redis as redis_lib
        from rq import Queue
        from datetime import timedelta

        r = redis_lib.from_url(redis_url)
        q = Queue(kind, connection=r)
        worker_fn = run_transcribe_job  # always transcribe in this module
        rq_job = q.enqueue_in(timedelta(seconds=delay_sec), worker_fn, job_id, slug, params, job_timeout=3600)

        from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo
        SqliteJobRepo(db_path).set_rq_job_id(job_id, rq_job.id)
        _log.info("Scheduled retry rq=%s for job=%s in %ds", rq_job.id, job_id, delay_sec)
    except Exception:
        _log.exception("Failed to schedule retry for job %s", job_id)


def _enqueue_protocol(
    slug: str, transcribe_job_id: str, params: dict, redis_url: str, db_path: Path
) -> None:
    try:
        import redis as redis_lib
        from rq import Queue
        from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo
        from meeting_assistant.infrastructure.container import build_container
        from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
        from meeting_assistant.domain.value_objects.meeting_status import MeetingStatus
        from .protocol_worker import run_protocol_job

        r = redis_lib.from_url(redis_url)
        job_repo = SqliteJobRepo(db_path)
        protocol_params = {
            "template": params.get("template"),
            "auto_protocol": False,
        }
        job = job_repo.create(slug, "protocol", protocol_params)

        container = build_container()
        try:
            container.meeting_repo.update_status(MeetingSlug(slug), MeetingStatus.GENERATING)
        except Exception:
            pass

        q = Queue("protocol", connection=r)
        rq_job = q.enqueue(run_protocol_job, job.id, slug, protocol_params, job_timeout=1800)
        job_repo.set_rq_job_id(job.id, rq_job.id)
    except Exception:
        _log.exception("Failed to auto-enqueue protocol job for %s", slug)
