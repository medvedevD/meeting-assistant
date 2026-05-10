import math
import re
import signal
import struct
import subprocess
import time
from pathlib import Path
from fastapi import APIRouter, Request, HTTPException
from ..schemas import RecordStartRequest, RecordStopRequest

router = APIRouter()

_CACHE_DIR = Path.home() / ".cache/meeting-assistant/recording"
_WAV_HEADER = 44
_SAMPLE_RATE = 16000
_BYTES_PER_SAMPLE = 2  # pcm_s16le


def _get_mic_source(container) -> str:
    try:
        return container.config.load().get("recording", {}).get("mic_source", "")
    except Exception:
        return ""


def _is_mic_muted(source: str) -> bool:
    try:
        if not source:
            source = subprocess.check_output(
                ["pactl", "get-default-source"], text=True, timeout=2
            ).strip()
        out = subprocess.check_output(
            ["pactl", "get-source-mute", source], text=True, timeout=2,
            env={**__import__("os").environ, "LANG": "C"},
        )
        return "yes" in out.lower()
    except Exception:
        return False


def _live_wav_max_db(window_secs: float = 2.0) -> float | None:
    """Peak dB over the last `window_secs` of the current live recording, or None."""
    if not _CACHE_DIR.exists():
        return None
    files = sorted(_CACHE_DIR.glob("current-*.wav"), key=lambda f: f.stat().st_mtime, reverse=True)
    if not files:
        return None
    read_bytes = int(window_secs * _SAMPLE_RATE * _BYTES_PER_SAMPLE)
    try:
        with open(files[0], "rb") as f:
            total = f.seek(0, 2)
            available = total - _WAV_HEADER
            if available <= 0:
                return None
            to_read = min(read_bytes, available) & ~1  # keep even
            f.seek(-to_read, 2)
            data = f.read(to_read)
        samples = struct.unpack(f"<{len(data) // 2}h", data)
        peak = max(abs(s) for s in samples)
        return 20 * math.log10(peak / 32767.0) if peak > 0 else -96.0
    except Exception:
        return None


@router.get("/record/mic-mute")
def get_mic_mute(source: str = "", request: Request = None):
    return {"muted": _is_mic_muted(source)}


@router.get("/record/audio-level")
def get_audio_level(request: Request):
    ws = request.app.state.web_state
    with ws.rec_lock:
        recording = ws.rec_proc is not None and ws.rec_proc.poll() is None
    if not recording:
        return {"recording": False}
    max_db = _live_wav_max_db()
    return {"recording": True, "max_db": max_db}


@router.get("/record/status")
def get_rec_status(request: Request):
    ws = request.app.state.web_state
    with ws.rec_lock:
        running = ws.rec_proc is not None and ws.rec_proc.poll() is None
        elapsed = int(time.time() - ws.rec_start) if running and ws.rec_start else 0
        return {"running": running, "elapsed": elapsed, "folder": ws.rec_folder}


@router.post("/record/start")
def post_rec_start(data: RecordStartRequest, request: Request):
    ws = request.app.state.web_state
    scripts_dir = request.app.state.container.scripts_dir

    raw = data.name.strip()
    if raw and data.prepend_date and not re.match(r"^\d{4}-\d{2}-\d{2}", raw):
        raw = time.strftime("%Y-%m-%d_%H-%M") + "_" + raw
    name = raw or time.strftime("%Y-%m-%d_%H-%M")

    with ws.rec_lock:
        if ws.rec_proc and ws.rec_proc.poll() is None:
            raise HTTPException(400, "already recording")
        ws.rec_folder = name
        ws.rec_proc = subprocess.Popen(
            [str(scripts_dir / "record-meeting.sh"), name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        ws.rec_start = time.time()

    return {"ok": True, "folder": name}


@router.post("/record/stop")
def post_rec_stop(data: RecordStopRequest, request: Request):
    ws = request.app.state.web_state
    meetings_dir = request.app.state.container.meeting_repo.meetings_dir

    new_name = re.sub(r"[^\w\-]", "_", data.name.strip())
    with ws.rec_lock:
        if ws.rec_proc and ws.rec_proc.poll() is None:
            ws.rec_proc.send_signal(signal.SIGTERM)
            try:
                ws.rec_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                ws.rec_proc.kill()
        old_folder = ws.rec_folder or ""
        if new_name and new_name != old_folder:
            old_path = meetings_dir / old_folder
            new_path = meetings_dir / new_name
            if old_path.exists() and not new_path.exists():
                old_path.rename(new_path)
                ws.rec_folder = new_name

    return {"ok": True, "folder": ws.rec_folder}
