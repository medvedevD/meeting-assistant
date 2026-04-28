import re
import signal
import subprocess
import time
from pathlib import Path
from fastapi import APIRouter, Request, HTTPException
from ..schemas import RecordStartRequest, RecordStopRequest

router = APIRouter()


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
            ["pactl", "get-source-mute", source], text=True, timeout=2
        )
        return "yes" in out.lower()
    except Exception:
        return False


def _is_audio_silent(wav_path: Path, threshold_db: float = -40.0) -> bool:
    try:
        result = subprocess.run(
            ["ffmpeg", "-i", str(wav_path), "-af", "volumedetect", "-f", "null", "/dev/null"],
            capture_output=True, text=True, timeout=60,
        )
        m = re.search(r"max_volume:\s*([-\d.]+)\s*dB", result.stderr)
        if m:
            return float(m.group(1)) < threshold_db
    except Exception:
        pass
    return False


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

    mic_source = _get_mic_source(request.app.state.container)
    warning = "mic_muted" if _is_mic_muted(mic_source) else None
    return {"ok": True, "folder": name, "warning": warning}


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

    warning = None
    folder = ws.rec_folder
    if folder:
        wav = meetings_dir / folder / "recording.wav"
        # ffmpeg needs a moment to flush before we can read the file
        deadline = time.time() + 3.0
        while not wav.exists() and time.time() < deadline:
            time.sleep(0.1)
        if wav.exists():
            warning = "silent_audio" if _is_audio_silent(wav) else None

    return {"ok": True, "folder": ws.rec_folder, "warning": warning}
