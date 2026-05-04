import asyncio
import json
import os
import queue as q_module
import re
import subprocess
import threading
from pathlib import Path
from fastapi import APIRouter, Request, HTTPException
from fastapi.responses import StreamingResponse
from ..schemas import ProcessStartRequest
from ..sse import ProcessingEvent

router = APIRouter()

_USE_RQ = os.environ.get("JOB_RUNNER", "").lower() == "rq"

_SENTINEL = object()

_FOLDER_RE = re.compile(r"^[\w\-\. ]+$")
_STAGE_RE = re.compile(r"^__STAGE:(\w+)__$")
_PROGRESS_RE = re.compile(r"^__PROGRESS:([\d.]+)__$")


@router.get("/process/stream")
async def get_proc_stream(request: Request):
    ws = request.app.state.web_state

    if _USE_RQ:
        return await _rq_proc_stream(request, ws)

    loop = asyncio.get_running_loop()

    async def event_generator():
        while True:
            if await request.is_disconnected():
                break
            try:
                item = await loop.run_in_executor(
                    None, lambda: ws.proc_queue.get(timeout=1)
                )
                if item is _SENTINEL:
                    ws.proc_queue.put(_SENTINEL)
                    yield ProcessingEvent(stage="done").to_sse()
                    break
                if isinstance(item, ProcessingEvent):
                    yield item.to_sse()
                else:
                    line = str(item).rstrip("\n").strip()
                    if line:
                        yield ProcessingEvent(stage="processing", message=line).to_sse()
            except q_module.Empty:
                yield ": ping\n\n"

    return StreamingResponse(
        event_generator(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


# Map worker stage names to frontend stage names (None = skip)
_STAGE_MAP = {
    "loading_model": "save",
    "transcribing": "transcribe",
    "loading_transcript": None,
    "generating": "protocol",
    "saving": None,
}


async def _rq_proc_stream(request: Request, ws):
    """Bridge Redis pub/sub events to the legacy SSE format."""
    runner = request.app.state.container.job_runner
    if runner is None:
        raise HTTPException(503, "RQ pipeline not configured")

    # Prefer slug from query param; fall back to ws state for clients that don't pass it
    slug = request.query_params.get("slug") or ws.rq_current_slug
    if not slug:
        # Wait up to 10 s for POST /process/start to set ws.rq_current_slug
        for _ in range(100):
            if await request.is_disconnected():
                return StreamingResponse(iter([]), media_type="text/event-stream")
            slug = ws.rq_current_slug
            if slug:
                break
            await asyncio.sleep(0.1)

    async def event_generator():
        nonlocal slug
        if not slug:
            yield ProcessingEvent(stage="error", message="Не удалось определить задачу").to_sse()
            yield ProcessingEvent(stage="done").to_sse()
            return

        loop = asyncio.get_running_loop()
        pubsub = await loop.run_in_executor(None, runner.subscribe_events, slug)
        last_stage = None
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
                        ev_type = payload.get("type")

                        if ev_type == "progress":
                            raw_stage = payload.get("stage")
                            value = payload.get("value")
                            ui_stage = _STAGE_MAP.get(raw_stage, raw_stage)
                            if ui_stage and ui_stage != last_stage:
                                last_stage = ui_stage
                                yield ProcessingEvent(stage=ui_stage).to_sse()
                            if value is not None:
                                yield ProcessingEvent(stage="progress", progress=float(value)).to_sse()
                        elif ev_type == "transcription_done":
                            # Mid-pipeline: transcription done, protocol starting next
                            last_stage = "protocol"
                            yield ProcessingEvent(stage="protocol").to_sse()
                        elif ev_type == "job_done":
                            yield ProcessingEvent(stage="done").to_sse()
                            break
                        elif ev_type in ("error", "cancelled"):
                            text = payload.get("message") or "Обработка остановлена"
                            yield ProcessingEvent(stage="error", message=text).to_sse()
                            yield ProcessingEvent(stage="done").to_sse()
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


def _find_recording(folder_path: Path) -> Path:
    recordings = list(folder_path.glob("recording.*"))
    if recordings:
        return recordings[0]
    return folder_path / "recording.wav"


def start_processing_job(ws, container, audio_path: str, data: ProcessStartRequest) -> None:
    """Spawn a background thread that runs process_meeting.py and feeds ws.proc_queue."""
    with ws.proc_lock:
        if ws.proc_running:
            raise HTTPException(400, "already processing")
        ws.proc_running = True

    while not ws.proc_queue.empty():
        try:
            ws.proc_queue.get_nowait()
        except q_module.Empty:
            break

    def run():
        venv_python = container.root_dir / ".venv" / "bin" / "python3"
        cmd = [str(venv_python), "-u", str(container.scripts_dir / "process_meeting.py"), audio_path]
        if data.name:
            cmd.append(data.name)
        if data.model:
            cmd += ["--model", data.model]
        if data.no_protocol:
            cmd.append("--no-protocol")
        if data.from_transcript:
            cmd.append("--from-transcript")

        env = os.environ.copy()
        api_cfg = container.config.load()["api"]
        for env_key, cfg_key in [
            ("ANTHROPIC_API_KEY", "anthropic_api_key"),
            ("GEMINI_API_KEY", "gemini_api_key"),
            ("MISTRAL_API_KEY", "mistral_api_key"),
        ]:
            if not env.get(env_key):
                key = api_cfg.get(cfg_key, "")
                if key and key != "***":
                    env[env_key] = key

        try:
            proc = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
                env=env,
            )
            with ws.proc_lock:
                ws.proc_proc = proc
            for line in proc.stdout:
                stripped = line.strip()
                if m := _STAGE_RE.match(stripped):
                    ws.proc_queue.put(ProcessingEvent(stage=m.group(1)))
                elif m := _PROGRESS_RE.match(stripped):
                    ws.proc_queue.put(ProcessingEvent(stage="progress", progress=float(m.group(1))))
                else:
                    ws.proc_queue.put(line)
            proc.wait()
            # Sync DB status from filesystem (legacy pipeline writes files directly)
            try:
                container.meeting_repo.backfill()
            except Exception:
                pass
        except Exception as e:
            ws.proc_queue.put(f"Ошибка: {e}\n")
        finally:
            ws.proc_queue.put(_SENTINEL)
            with ws.proc_lock:
                ws.proc_running = False
                ws.proc_proc = None

    threading.Thread(target=run, daemon=True).start()


@router.post("/process/cancel")
def post_proc_cancel(request: Request):
    ws = request.app.state.web_state
    with ws.proc_lock:
        proc = ws.proc_proc
        if proc is None:
            raise HTTPException(404, "no active process")
        try:
            proc.terminate()
        except Exception:
            pass
    return {"ok": True}


@router.post("/process/start")
def post_proc_start(data: ProcessStartRequest, request: Request):
    ws = request.app.state.web_state
    container = request.app.state.container
    meetings_dir = container.meeting_repo.meetings_dir

    if not _FOLDER_RE.match(data.folder):
        raise HTTPException(400, "invalid folder")
    meeting_dir = (meetings_dir / data.folder).resolve()
    if not str(meeting_dir).startswith(str(meetings_dir)):
        raise HTTPException(400, "invalid path")

    if _USE_RQ:
        coord = container.pipeline_coordinator
        if coord is None:
            raise HTTPException(503, "RQ pipeline not configured")
        # Register any newly recorded meeting folders not yet in the DB
        container.meeting_repo.backfill()
        from ....application.orchestration.pipeline_coordinator import PipelineError
        pipe_params = {}
        if data.model:
            pipe_params["model"] = data.model
        if data.template:
            pipe_params["template"] = data.template
        if data.provider:
            pipe_params["provider"] = data.provider
        if data.llm_model:
            pipe_params["llm_model"] = data.llm_model
        try:
            if data.from_transcript:
                job_id = coord.start_protocol(data.folder, pipe_params)
            elif data.no_protocol:
                job_id = coord.start_transcription(data.folder, pipe_params)
            else:
                job_id = coord.start_full_pipeline(data.folder, pipe_params)
        except PipelineError as e:
            raise HTTPException(400, str(e))
        ws.rq_current_slug = data.folder
        return {"ok": True, "job_id": job_id}

    from ....application.validate_audio import validate_audio

    audio_path = _find_recording(meeting_dir)

    if not data.from_transcript:
        validation = validate_audio(audio_path)
        if not validation.ok:
            raise HTTPException(422, detail=validation.error_message or "Ошибка аудиофайла")

    start_processing_job(ws, container, str(audio_path), data)
    return {"ok": True}
