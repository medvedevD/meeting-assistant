"""
PipelineCoordinator — orchestrates pipeline transitions + job enqueueing.

All POST /api/meetings/{slug}/transcribe|generate go through here.
Status transitions and job creation are done atomically (best-effort).
"""
import json
import logging

from ...adapters.storage.sqlite_job_repo import SqliteJobRepo
from ...domain.ports.job_runner import IJobRunner
from ...domain.ports.meeting_repository import IMeetingRepository
from ...domain.services.state_machine import InvalidTransitionError
from ...domain.value_objects.meeting_slug import MeetingSlug
from ...domain.value_objects.meeting_status import MeetingStatus

_log = logging.getLogger(__name__)


class PipelineError(Exception):
    pass


class PipelineCoordinator:
    def __init__(
        self,
        meeting_repo: IMeetingRepository,
        job_repo: SqliteJobRepo,
        job_runner: IJobRunner,
    ):
        self._repo = meeting_repo
        self._job_repo = job_repo
        self._runner = job_runner

    # ── public API ────────────────────────────────────────────────────────────

    def start_transcription(self, slug: str, params: dict | None = None) -> str:
        """
        Transition meeting to TRANSCRIBING and enqueue a transcribe job.
        Returns the internal job ID.
        """
        params = dict(params or {})
        params.setdefault("auto_protocol", False)  # explicit call → no auto-chain

        meeting_slug = MeetingSlug(slug)
        meeting = self._repo.get(meeting_slug)
        if meeting is None:
            raise PipelineError(f"Meeting {slug!r} not found")

        try:
            self._repo.update_status(meeting_slug, MeetingStatus.TRANSCRIBING)
        except InvalidTransitionError as e:
            raise PipelineError(str(e)) from e

        job = self._job_repo.create(slug, "transcribe", params)
        try:
            rq_id = self._runner.enqueue_transcribe(job.id, slug, params)
            self._job_repo.set_rq_job_id(job.id, rq_id)
        except Exception as exc:
            self._job_repo.mark_failed(job.id, str(exc), "unknown")
            try:
                self._repo.update_status(meeting_slug, MeetingStatus.TRANSCRIBE_FAILED)
            except Exception:
                pass
            raise PipelineError(f"Failed to enqueue transcription: {exc}") from exc

        return job.id

    def start_protocol(self, slug: str, params: dict | None = None) -> str:
        """
        Transition meeting to GENERATING and enqueue a protocol job.
        Returns the internal job ID.
        """
        params = dict(params or {})

        meeting_slug = MeetingSlug(slug)
        meeting = self._repo.get(meeting_slug)
        if meeting is None:
            raise PipelineError(f"Meeting {slug!r} not found")

        try:
            self._repo.update_status(meeting_slug, MeetingStatus.GENERATING)
        except InvalidTransitionError as e:
            raise PipelineError(str(e)) from e

        job = self._job_repo.create(slug, "protocol", params)
        try:
            rq_id = self._runner.enqueue_protocol(job.id, slug, params)
            self._job_repo.set_rq_job_id(job.id, rq_id)
        except Exception as exc:
            self._job_repo.mark_failed(job.id, str(exc), "unknown")
            try:
                self._repo.update_status(meeting_slug, MeetingStatus.PROTOCOL_FAILED)
            except Exception:
                pass
            raise PipelineError(f"Failed to enqueue protocol generation: {exc}") from exc

        return job.id

    def start_full_pipeline(self, slug: str, params: dict | None = None) -> str:
        """
        Enqueue transcription with auto_protocol=True so the worker
        auto-chains protocol generation upon success.
        Returns the transcribe job ID.
        """
        params = dict(params or {})
        params["auto_protocol"] = True
        return self.start_transcription(slug, params)

    def cancel_job(self, job_id: str) -> None:
        job = self._job_repo.get(job_id)
        if job is None:
            raise PipelineError(f"Job {job_id!r} not found")
        if job.status not in ("queued", "running"):
            raise PipelineError(f"Job {job_id!r} is not active (status={job.status})")

        if job.rq_job_id:
            self._runner.cancel(job.rq_job_id)

        self._job_repo.mark_cancelled(job_id)

        # Roll back meeting status to the last stable state
        meeting_slug = MeetingSlug(job.meeting_slug)
        meeting = self._repo.get(meeting_slug)
        if job.kind == "transcribe":
            rollback = MeetingStatus.TRANSCRIBED if (meeting and meeting.has_transcript) else MeetingStatus.AUDIO_READY
        else:
            rollback = MeetingStatus.TRANSCRIBED
        try:
            self._repo.update_status(meeting_slug, rollback)
        except Exception:
            pass

    def retry_job(self, job_id: str) -> str:
        """
        Create a fresh job based on the failed job's params, re-enqueue.
        Returns new job ID.
        """
        original = self._job_repo.get(job_id)
        if original is None:
            raise PipelineError(f"Job {job_id!r} not found")
        if original.status not in ("failed", "cancelled"):
            raise PipelineError(f"Job {job_id!r} cannot be retried (status={original.status})")

        params = json.loads(original.params_json or "{}")
        slug = original.meeting_slug

        if original.kind == "transcribe":
            return self.start_transcription(slug, params)
        else:
            return self.start_protocol(slug, params)

    def reconcile(self) -> int:
        """
        On server start: find jobs that were running but RQ has no record of them.
        Re-enqueue them so they don't get stuck forever.
        Returns number of jobs re-queued.
        """
        running = self._job_repo.find_running()
        count = 0
        for job in running:
            if self._runner.is_alive(job.rq_job_id or ""):
                continue
            _log.warning("Reconcile: re-queuing orphaned job %s (slug=%s)", job.id, job.meeting_slug)
            params = json.loads(job.params_json or "{}")
            try:
                if job.kind == "transcribe":
                    new_rq_id = self._runner.enqueue_transcribe(job.id, job.meeting_slug, params)
                else:
                    new_rq_id = self._runner.enqueue_protocol(job.id, job.meeting_slug, params)
                self._job_repo.requeue(job.id, new_rq_id)
                count += 1
            except Exception:
                _log.exception("Failed to re-queue job %s", job.id)
        return count

    def get_active_jobs(self) -> list:
        return self._job_repo.find_active()

    def get_jobs_for_meeting(self, slug: str) -> list:
        return self._job_repo.find_by_slug(slug)
