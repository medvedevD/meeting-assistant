"""
Watchdog daemon — runs as a background asyncio task inside the web server.

Two responsibilities:
  1. Stale-job detection: every 60 s find jobs with status=running whose
     last_heartbeat_at is older than 120 s (worker likely dead).  Mark them
     failed/worker_killed and auto-retry if attempt < max_attempts.

  2. Recording limit: warn at 4 h, auto-stop at 8 h to prevent runaway
     recording processes.
"""

import asyncio
import json
import logging
from datetime import datetime, timedelta
from pathlib import Path

_log = logging.getLogger(__name__)

_STALE_THRESHOLD_SEC = 120      # seconds without heartbeat → worker dead
_CHECK_INTERVAL_SEC = 60        # how often to run the watchdog loop
_REC_WARN_SEC = 4 * 3600        # warn user at 4 h
_REC_STOP_SEC = 8 * 3600        # force-stop recording at 8 h


async def run_watchdog(container, web_state=None) -> None:
    """
    Long-running coroutine.  Pass ``web_state`` to enable the recording watchdog.
    Runs until the event loop is stopped.
    """
    while True:
        await asyncio.sleep(_CHECK_INTERVAL_SEC)
        try:
            await asyncio.get_event_loop().run_in_executor(
                None, _check_stale_jobs, container
            )
        except Exception:
            _log.exception("Watchdog: stale-job check failed")

        if web_state is not None:
            try:
                await asyncio.get_event_loop().run_in_executor(
                    None, _check_recording_limit, web_state
                )
            except Exception:
                _log.exception("Watchdog: recording-limit check failed")


# ── stale-job detection ───────────────────────────────────────────────────────

def _check_stale_jobs(container) -> None:
    from meeting_assistant.adapters.storage.sqlite_job_repo import SqliteJobRepo
    from meeting_assistant.adapters.jobs.error_classification import RETRY_BACKOFF
    from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
    from meeting_assistant.domain.value_objects.meeting_status import MeetingStatus

    job_repo: SqliteJobRepo = getattr(container, "job_repo", None)
    job_runner = getattr(container, "job_runner", None)
    meeting_repo = getattr(container, "meeting_repo", None)

    if job_repo is None or meeting_repo is None:
        return  # RQ not configured

    stale = job_repo.find_stale(stale_after_sec=_STALE_THRESHOLD_SEC)
    if not stale:
        return

    for job in stale:
        _log.warning(
            "Watchdog: stale job %s (slug=%s kind=%s attempt=%d/%d) — marking failed",
            job.id, job.meeting_slug, job.kind, job.attempt, job.max_attempts,
        )
        job_repo.mark_failed(job.id, "Worker killed (no heartbeat)", "worker_killed")

        # Roll back meeting status
        slug = MeetingSlug(job.meeting_slug)
        rollback = (
            MeetingStatus.TRANSCRIBE_FAILED if job.kind == "transcribe"
            else MeetingStatus.PROTOCOL_FAILED
        )
        try:
            meeting_repo.update_status(slug, rollback)
        except Exception:
            pass

        # Publish error event so SSE clients see the update
        _publish_error(container, job.meeting_slug, job.id, "worker_killed")

        # Auto-retry if under max attempts
        if job.attempt < job.max_attempts and job_runner is not None:
            params = json.loads(job.params_json or "{}")
            delay = RETRY_BACKOFF[min(job.attempt, len(RETRY_BACKOFF) - 1)]
            job_repo.increment_attempt(job.id)
            _log.info(
                "Watchdog: re-queuing job %s in %ds (attempt %d/%d)",
                job.id, delay, job.attempt + 1, job.max_attempts,
            )
            try:
                if job.kind == "transcribe":
                    new_rq_id = job_runner.enqueue_transcribe_delayed(
                        job.id, job.meeting_slug, params, delay
                    )
                else:
                    new_rq_id = job_runner.enqueue_protocol_delayed(
                        job.id, job.meeting_slug, params, delay
                    )
                job_repo.set_rq_job_id(job.id, new_rq_id)
                # Mark queued again so UI shows it's waiting
                job_repo.requeue(job.id, new_rq_id)
            except Exception:
                _log.exception("Watchdog: failed to re-queue job %s", job.id)


def _publish_error(container, slug: str, job_id: str, error_kind: str) -> None:
    """Publish an error event to the meeting's Redis pub/sub channel."""
    try:
        import redis
        import os
        redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
        r = redis.from_url(redis_url)
        import json as _json
        r.publish(
            f"meeting:{slug}:events",
            _json.dumps({
                "type": "error",
                "error_kind": error_kind,
                "message": "Worker killed (no heartbeat). Will retry automatically.",
            }),
        )
    except Exception:
        pass


# ── recording limit ───────────────────────────────────────────────────────────

def _check_recording_limit(web_state) -> None:
    """Force-stop a recording that has been running for more than _REC_STOP_SEC."""
    import signal
    import subprocess

    with web_state.rec_lock:
        proc = web_state.rec_proc
        rec_start = web_state.rec_start
        if proc is None or proc.poll() is not None:
            return
        elapsed = (datetime.now() - datetime.fromtimestamp(rec_start)).total_seconds()

    if elapsed >= _REC_STOP_SEC:
        _log.warning(
            "Watchdog: recording running for %.0f h — force-stopping", elapsed / 3600
        )
        with web_state.rec_lock:
            proc = web_state.rec_proc
            if proc and proc.poll() is None:
                try:
                    proc.send_signal(signal.SIGTERM)
                    proc.wait(timeout=5)
                except (subprocess.TimeoutExpired, OSError):
                    try:
                        proc.kill()
                    except OSError:
                        pass
                web_state.rec_proc = None
        # Publish a warning so the UI can show a toast
        _publish_recording_stopped(web_state, elapsed)
    elif elapsed >= _REC_WARN_SEC:
        # Just log; the UI polling /record/status will pick this up via elapsed
        _log.info(
            "Watchdog: recording has been running for %.1f h (warning threshold)",
            elapsed / 3600,
        )


def _publish_recording_stopped(web_state, elapsed_sec: float) -> None:
    """Best-effort publish of a 'recording_auto_stopped' event."""
    try:
        import redis
        import os
        import json as _json
        folder = getattr(web_state, "rec_folder", "")
        redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379")
        r = redis.from_url(redis_url)
        r.publish(
            f"meeting:{folder}:events",
            _json.dumps({
                "type": "recording_auto_stopped",
                "elapsed_sec": elapsed_sec,
                "reason": "max_duration_exceeded",
            }),
        )
    except Exception:
        pass
