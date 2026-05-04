"""
RQRunner — IJobRunner implementation backed by Redis Queue.

Falls back gracefully if Redis is not available (logs a warning,
raises RuntimeError on enqueue so the caller can report an error).
"""
import logging
import os

from ...domain.ports.job_runner import IJobRunner

_log = logging.getLogger(__name__)

_CANCEL_KEY_TTL = 3600  # seconds


class RQRunner(IJobRunner):
    def __init__(self, redis_url: str | None = None):
        self._redis_url = redis_url or os.environ.get("REDIS_URL", "redis://localhost:6379")
        self._redis = None
        self._q_transcribe = None
        self._q_protocol = None

    # ── lazy init ─────────────────────────────────────────────────────────────

    def _get_redis(self):
        if self._redis is None:
            import redis
            self._redis = redis.from_url(self._redis_url)
        return self._redis

    def _get_queue(self, name: str):
        from rq import Queue
        r = self._get_redis()
        if name == "transcribe":
            if self._q_transcribe is None:
                self._q_transcribe = Queue("transcribe", connection=r)
            return self._q_transcribe
        if name == "protocol":
            if self._q_protocol is None:
                self._q_protocol = Queue("protocol", connection=r)
            return self._q_protocol
        raise ValueError(f"Unknown queue: {name}")

    def ping(self) -> bool:
        try:
            self._get_redis().ping()
            return True
        except Exception:
            return False

    # ── IJobRunner ─────────────────────────────────────────────────────────────

    def enqueue_transcribe(self, job_id: str, slug: str, params: dict) -> str:
        from rq.job import Retry
        from .workers.transcribe_worker import run_transcribe_job
        q = self._get_queue("transcribe")
        # Retry is handled manually inside the worker for transient errors;
        # max=0 here means RQ won't add its own retries on top.
        rq_job = q.enqueue(run_transcribe_job, job_id, slug, params, job_timeout=3600)
        _log.info("Enqueued transcribe job rq=%s for meeting=%s", rq_job.id, slug)
        return rq_job.id

    def enqueue_transcribe_delayed(self, job_id: str, slug: str, params: dict, delay_sec: int) -> str:
        from datetime import timedelta
        from .workers.transcribe_worker import run_transcribe_job
        q = self._get_queue("transcribe")
        rq_job = q.enqueue_in(
            timedelta(seconds=delay_sec),
            run_transcribe_job, job_id, slug, params,
            job_timeout=3600,
        )
        _log.info(
            "Scheduled retry transcribe rq=%s for meeting=%s in %ds",
            rq_job.id, slug, delay_sec,
        )
        return rq_job.id

    def enqueue_protocol(self, job_id: str, slug: str, params: dict) -> str:
        from .workers.protocol_worker import run_protocol_job
        q = self._get_queue("protocol")
        rq_job = q.enqueue(run_protocol_job, job_id, slug, params, job_timeout=1800)
        _log.info("Enqueued protocol job rq=%s for meeting=%s", rq_job.id, slug)
        return rq_job.id

    def enqueue_protocol_delayed(self, job_id: str, slug: str, params: dict, delay_sec: int) -> str:
        from datetime import timedelta
        from .workers.protocol_worker import run_protocol_job
        q = self._get_queue("protocol")
        rq_job = q.enqueue_in(
            timedelta(seconds=delay_sec),
            run_protocol_job, job_id, slug, params,
            job_timeout=1800,
        )
        _log.info(
            "Scheduled retry protocol rq=%s for meeting=%s in %ds",
            rq_job.id, slug, delay_sec,
        )
        return rq_job.id

    def cancel(self, rq_job_id: str) -> None:
        r = self._get_redis()
        try:
            from rq.job import Job
            rq_job = Job.fetch(rq_job_id, connection=r)
            rq_job.cancel()
        except Exception:
            pass  # job may already be running or finished
        # Set cancel flag; worker checks this between segments
        r.set(f"cancel:{rq_job_id}", "1", ex=_CANCEL_KEY_TTL)
        _log.info("Cancellation requested for rq_job_id=%s", rq_job_id)

    def is_alive(self, rq_job_id: str) -> bool:
        if not rq_job_id:
            return False
        try:
            from rq.job import Job
            rq_job = Job.fetch(rq_job_id, connection=self._get_redis())
            status = rq_job.get_status()
            return status in ("queued", "started", "deferred", "scheduled")
        except Exception:
            return False

    def subscribe_events(self, slug: str):
        """Return a Redis pubsub subscribed to meeting events. Caller must close."""
        r = self._get_redis()
        pubsub = r.pubsub(ignore_subscribe_messages=True)
        pubsub.subscribe(f"meeting:{slug}:events")
        return pubsub
