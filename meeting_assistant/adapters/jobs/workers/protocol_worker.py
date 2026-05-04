"""
RQ worker function: protocol generation.

Called by RQ worker process as:
  rq worker protocol
"""
import logging
import os
import threading
from pathlib import Path

_log = logging.getLogger(__name__)

_HEARTBEAT_INTERVAL = 20  # seconds between heartbeats during LLM call


class _CancelledError(Exception):
    pass


def run_protocol_job(job_id: str, slug: str, params: dict) -> None:
    """
    Generate a meeting protocol from an existing transcript.

    params keys:
      template  — template name (overrides config default)
    """
    from meeting_assistant.infrastructure.container import build_container
    from meeting_assistant.adapters.jobs.progress_reporter import ProgressReporter
    from meeting_assistant.adapters.jobs.error_classification import (
        classify_error, RETRYABLE_ERRORS, RETRY_BACKOFF,
    )
    from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo
    from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
    from meeting_assistant.domain.value_objects.meeting_status import MeetingStatus

    redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
    container = build_container()
    db_path = container.meeting_repo._db_path
    job_repo = SqliteJobRepo(db_path)

    # Increment attempt counter before marking running.
    job_repo.increment_attempt(job_id)

    reporter = ProgressReporter(job_id=job_id, slug=slug, db_path=db_path, redis_url=redis_url)
    reporter.mark_running()

    meeting_slug = MeetingSlug(slug)
    meeting = container.meeting_repo.get(meeting_slug)
    if meeting is None:
        reporter.mark_failed(f"Meeting {slug} not found", "unknown")
        return

    try:
        reporter.update("loading_transcript", None)

        if reporter.should_cancel():
            raise _CancelledError("Job cancelled by user")

        transcript_text = container.meeting_repo.load_transcript_text(meeting_slug)
        instructions = _build_instructions(container, transcript_text, meeting.title, params)

        reporter.update("generating", None)

        if reporter.should_cancel():
            raise _CancelledError("Job cancelled by user")

        meeting_dir = container.meeting_repo.get_meeting_dir(meeting_slug)
        partial_save_path = meeting_dir / "protocol.md"

        # Keep heartbeat alive during the potentially long LLM call.
        stop_hb = threading.Event()
        hb_thread = threading.Thread(
            target=_heartbeat_loop,
            args=(reporter, stop_hb, _HEARTBEAT_INTERVAL),
            daemon=True,
        )
        hb_thread.start()
        try:
            content = container.llm_provider.generate(
                transcript_text, instructions, partial_save_path=partial_save_path,
                provider=params.get("provider") or None,
                model=params.get("llm_model") or None,
            )
        finally:
            stop_hb.set()
            hb_thread.join(timeout=2)

        if reporter.should_cancel():
            raise _CancelledError("Job cancelled by user")

        reporter.update("saving", None)
        from datetime import datetime
        date_str = datetime.now().strftime("%d.%m.%Y")
        container.meeting_repo.save_protocol(meeting_slug, meeting.title, content, date_str)

        # Persist the model/template used so the card can show it
        try:
            prov = params.get("provider") or container.config.get("protocol", "provider", "claude")
            llm_m = params.get("llm_model") or container.config.get("protocol", f"{prov}_model", "")
            tmpl = params.get("template") or container.config.get("protocol", "active_template", "")
            container.meeting_repo.update_processing_metadata(
                meeting_slug,
                protocol_model=f"{prov}:{llm_m}" if llm_m else prov,
                template_name=tmpl or None,
            )
        except Exception:
            pass

        # Publish protocol-ready event to search index
        from meeting_assistant.application.events import ProtocolGenerated
        protocol_path = meeting_dir / "protocol.md"
        container.event_bus.publish(
            ProtocolGenerated(slug=meeting_slug, protocol_path=str(protocol_path), model="")
        )

        reporter.mark_succeeded()
        reporter._publish({"type": "meeting_status", "status": "COMPLETED"})

    except _CancelledError:
        _log.info("Protocol generation cancelled for %s", slug)
        try:
            container.meeting_repo.update_status(meeting_slug, MeetingStatus.TRANSCRIBED)
        except Exception:
            pass
        reporter.mark_cancelled()

    except Exception as exc:
        _log.exception("Protocol generation failed for %s", slug)
        error_kind = classify_error(exc)
        try:
            container.meeting_repo.update_status(meeting_slug, MeetingStatus.PROTOCOL_FAILED)
        except Exception:
            pass
        reporter.mark_failed(str(exc), error_kind)

        if error_kind in RETRYABLE_ERRORS:
            job = job_repo.get(job_id)
            if job and job.attempt < job.max_attempts:
                delay = RETRY_BACKOFF[min(job.attempt - 1, len(RETRY_BACKOFF) - 1)]
                _log.info(
                    "Auto-retry protocol for %s in %ds (attempt %d/%d)",
                    slug, delay, job.attempt, job.max_attempts,
                )
                _schedule_retry(slug, job_id, params, redis_url, db_path, delay)
                return  # handled — don't re-raise
        raise


def _heartbeat_loop(reporter: "ProgressReporter", stop: threading.Event, interval: int) -> None:
    """Send periodic heartbeats while a long operation is in progress."""
    while not stop.wait(timeout=interval):
        try:
            reporter.heartbeat()
        except Exception:
            pass


def _schedule_retry(
    slug: str,
    job_id: str,
    params: dict,
    redis_url: str,
    db_path: Path,
    delay_sec: int,
) -> None:
    try:
        import redis as redis_lib
        from rq import Queue
        from datetime import timedelta
        from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo

        r = redis_lib.from_url(redis_url)
        q = Queue("protocol", connection=r)
        rq_job = q.enqueue_in(
            timedelta(seconds=delay_sec),
            run_protocol_job, job_id, slug, params,
            job_timeout=1800,
        )
        SqliteJobRepo(db_path).set_rq_job_id(job_id, rq_job.id)
        _log.info("Scheduled retry rq=%s for job=%s in %ds", rq_job.id, job_id, delay_sec)
    except Exception:
        _log.exception("Failed to schedule retry for job %s", job_id)


def _build_instructions(container, transcript: str, meeting_name: str, params: dict) -> str | None:
    template_name = params.get("template") or container.config.get("protocol", "active_template", "")
    templates = container.template_repo.list()
    tmpl = next((t for t in templates if t["name"] == template_name), None)
    if not tmpl:
        return None
    pre, _, post = tmpl["prompt"].partition("{transcript}")
    return (pre + post).replace("{meeting_name}", meeting_name)
