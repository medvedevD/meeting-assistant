"""
Pipeline routes (RQ-backed):

  POST /api/meetings/{slug}/process    — full pipeline (transcribe + auto protocol)
  POST /api/meetings/{slug}/transcribe — only transcription
  POST /api/meetings/{slug}/generate   — only protocol generation
  POST /api/jobs/{job_id}/cancel
  POST /api/jobs/{job_id}/retry
  GET  /api/meetings/{slug}/stream     — SSE from Redis pub/sub
  GET  /api/meetings/{slug}/jobs       — list jobs for a meeting
  GET  /api/jobs/active                — all active jobs (queue UI)
"""
import asyncio
import json
import logging

from fastapi import APIRouter, HTTPException, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

from ....application.orchestration.pipeline_coordinator import PipelineCoordinator, PipelineError

_log = logging.getLogger(__name__)

router = APIRouter()


# ── helpers ──────────────────────────────────────────────────────────────────

def _coordinator(request: Request) -> PipelineCoordinator:
    c = getattr(request.app.state.container, "pipeline_coordinator", None)
    if c is None:
        raise HTTPException(503, "RQ pipeline not configured. Set JOB_RUNNER=rq and start Redis.")
    return c


def _job_to_dict(job) -> dict:
    return {
        "id": job.id,
        "meeting_slug": job.meeting_slug,
        "kind": job.kind,
        "status": job.status,
        "attempt": job.attempt,
        "max_attempts": job.max_attempts,
        "progress_stage": job.progress_stage,
        "progress_value": job.progress_value,
        "progress_eta_sec": job.progress_eta_sec,
        "error_message": job.error_message,
        "error_kind": job.error_kind,
        "enqueued_at": job.enqueued_at.isoformat() if job.enqueued_at else None,
        "started_at": job.started_at.isoformat() if job.started_at else None,
        "finished_at": job.finished_at.isoformat() if job.finished_at else None,
    }


# ── start pipeline ────────────────────────────────────────────────────────────

class PipelineParams(BaseModel):
    model: str | None = None
    template: str | None = None
    provider: str | None = None
    llm_model: str | None = None


@router.post("/meetings/{slug}/process")
def start_process(slug: str, data: PipelineParams, request: Request):
    """Full pipeline: transcription → auto protocol generation."""
    coord = _coordinator(request)
    params = {k: v for k, v in {
        "model": data.model, "template": data.template,
        "provider": data.provider, "llm_model": data.llm_model,
    }.items() if v}
    try:
        job_id = coord.start_full_pipeline(slug, params)
    except PipelineError as e:
        raise HTTPException(400, str(e))
    return {"ok": True, "job_id": job_id}


@router.post("/meetings/{slug}/transcribe")
def start_transcribe(slug: str, data: PipelineParams, request: Request):
    """Transcription only."""
    coord = _coordinator(request)
    params = {k: v for k, v in {"model": data.model}.items() if v}
    try:
        job_id = coord.start_transcription(slug, params)
    except PipelineError as e:
        raise HTTPException(400, str(e))
    return {"ok": True, "job_id": job_id}


@router.post("/meetings/{slug}/generate")
def start_generate(slug: str, data: PipelineParams, request: Request):
    """Protocol generation only (requires existing transcript)."""
    coord = _coordinator(request)
    params = {k: v for k, v in {
        "template": data.template, "provider": data.provider, "llm_model": data.llm_model,
    }.items() if v}
    try:
        job_id = coord.start_protocol(slug, params)
    except PipelineError as e:
        raise HTTPException(400, str(e))
    return {"ok": True, "job_id": job_id}


# ── job control ───────────────────────────────────────────────────────────────

@router.post("/jobs/{job_id}/cancel")
def cancel_job(job_id: str, request: Request):
    coord = _coordinator(request)
    try:
        coord.cancel_job(job_id)
    except PipelineError as e:
        raise HTTPException(400, str(e))
    return {"ok": True}


@router.post("/jobs/{job_id}/retry")
def retry_job(job_id: str, request: Request):
    coord = _coordinator(request)
    try:
        new_job_id = coord.retry_job(job_id)
    except PipelineError as e:
        raise HTTPException(400, str(e))
    return {"ok": True, "job_id": new_job_id}


# ── query ─────────────────────────────────────────────────────────────────────

@router.get("/meetings/{slug}/jobs")
def get_meeting_jobs(slug: str, request: Request):
    coord = _coordinator(request)
    jobs = coord.get_jobs_for_meeting(slug)
    return {"jobs": [_job_to_dict(j) for j in jobs]}


@router.get("/jobs/active")
def get_active_jobs(request: Request):
    coord = _coordinator(request)
    jobs = coord.get_active_jobs()
    return {"jobs": [_job_to_dict(j) for j in jobs]}


# ── SSE: per-meeting event stream from Redis pub/sub ─────────────────────────

@router.get("/meetings/{slug}/stream")
async def stream_meeting_events(slug: str, request: Request):
    """
    SSE endpoint that relays Redis pub/sub events for a specific meeting.
    Sends keep-alive pings every 5 s to detect disconnects.
    """
    runner = getattr(request.app.state.container, "job_runner", None)
    if runner is None:
        raise HTTPException(503, "RQ pipeline not configured")

    async def event_generator():
        loop = asyncio.get_running_loop()
        pubsub = await loop.run_in_executor(None, runner.subscribe_events, slug)
        try:
            while True:
                if await request.is_disconnected():
                    break
                msg = await loop.run_in_executor(
                    None, lambda: pubsub.get_message(timeout=5)
                )
                if msg and msg.get("type") == "message":
                    try:
                        data = msg["data"]
                        if isinstance(data, bytes):
                            data = data.decode()
                        payload = json.loads(data)
                        yield f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"
                        if payload.get("type") in ("job_done", "cancelled", "error"):
                            yield "event: done\ndata: {}\n\n"
                            break
                    except Exception:
                        pass
                else:
                    yield ": ping\n\n"
        finally:
            try:
                await loop.run_in_executor(None, pubsub.close)
            except Exception:
                pass

    return StreamingResponse(
        event_generator(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )
