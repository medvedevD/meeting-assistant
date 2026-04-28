import asyncio
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

_SENTINEL = object()

_FOLDER_RE = re.compile(r"^[\w\-\. ]+$")
_STAGE_RE = re.compile(r"^__STAGE:(\w+)__$")


@router.get("/process/stream")
async def get_proc_stream(request: Request):
    ws = request.app.state.web_state
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
            for line in proc.stdout:
                m = _STAGE_RE.match(line.strip())
                if m:
                    ws.proc_queue.put(ProcessingEvent(stage=m.group(1)))
                else:
                    ws.proc_queue.put(line)
            proc.wait()
        except Exception as e:
            ws.proc_queue.put(f"Ошибка: {e}\n")
        finally:
            ws.proc_queue.put(_SENTINEL)
            with ws.proc_lock:
                ws.proc_running = False

    threading.Thread(target=run, daemon=True).start()


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

    audio_path = str(_find_recording(meeting_dir))
    start_processing_job(ws, container, audio_path, data)
    return {"ok": True}
