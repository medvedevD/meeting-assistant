from abc import ABC, abstractmethod


class IJobRunner(ABC):
    @abstractmethod
    def enqueue_transcribe(self, job_id: str, slug: str, params: dict) -> str:
        """Enqueue a transcribe job. Returns the RQ job ID."""

    @abstractmethod
    def enqueue_transcribe_delayed(self, job_id: str, slug: str, params: dict, delay_sec: int) -> str:
        """Enqueue a transcribe job with an initial delay (for retry backoff)."""

    @abstractmethod
    def enqueue_protocol(self, job_id: str, slug: str, params: dict) -> str:
        """Enqueue a protocol generation job. Returns the RQ job ID."""

    @abstractmethod
    def enqueue_protocol_delayed(self, job_id: str, slug: str, params: dict, delay_sec: int) -> str:
        """Enqueue a protocol generation job with an initial delay (for retry backoff)."""

    @abstractmethod
    def cancel(self, rq_job_id: str) -> None:
        """Signal cancellation. Works for queued and running jobs."""

    @abstractmethod
    def is_alive(self, rq_job_id: str) -> bool:
        """Return True if the RQ job is queued or currently running."""
