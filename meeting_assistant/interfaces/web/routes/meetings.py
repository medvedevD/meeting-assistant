import re
import shutil
from pathlib import Path
from fastapi import APIRouter, Request, HTTPException, Query
from ..schemas import MeetingDeleteRequest

router = APIRouter()

_FOLDER_RE = re.compile(r"^[\w\-\. ]+$")


def _resolve_meeting_dir(folder: str, meetings_dir: Path) -> Path:
    if not _FOLDER_RE.match(folder):
        raise HTTPException(400, "invalid folder")
    meeting_dir = (meetings_dir / folder).resolve()
    if not str(meeting_dir).startswith(str(meetings_dir)):
        raise HTTPException(400, "invalid path")
    return meeting_dir


@router.get("/recordings")
def get_recordings(request: Request):
    d = request.app.state.container.meeting_repo.meetings_dir
    d.mkdir(exist_ok=True)
    folders = [
        p.name for p in sorted(d.iterdir(), reverse=True)
        if p.is_dir() and any(p.glob("recording.*"))
    ]
    return {"folders": folders}


@router.get("/meetings")
def get_meetings(request: Request):
    d = request.app.state.container.meeting_repo.meetings_dir
    d.mkdir(exist_ok=True)
    folders = []
    for p in sorted(d.iterdir(), reverse=True):
        if not p.is_dir():
            continue
        has_recording = any(p.glob("recording.*"))
        has_transcript = (p / "transcript.md").exists()
        has_protocol = (p / "protocol.md").exists()
        if has_recording or has_transcript or has_protocol:
            folders.append({
                "slug": p.name,
                "has_transcript": has_transcript,
                "has_protocol": has_protocol,
            })
    return {"folders": folders}


@router.get("/meeting")
def get_meeting(folder: str = Query(), request: Request = None):
    meetings_dir = request.app.state.container.meeting_repo.meetings_dir
    meeting_dir = _resolve_meeting_dir(folder, meetings_dir)
    protocol_path = meeting_dir / "protocol.md"
    transcript_path = meeting_dir / "transcript.md"
    return {
        "protocol": protocol_path.read_text(encoding="utf-8") if protocol_path.exists() else None,
        "transcript": transcript_path.read_text(encoding="utf-8") if transcript_path.exists() else None,
    }


@router.post("/meeting/delete")
def delete_meeting(data: MeetingDeleteRequest, request: Request):
    meetings_dir = request.app.state.container.meeting_repo.meetings_dir
    meeting_dir = _resolve_meeting_dir(data.folder, meetings_dir)
    if not meeting_dir.exists():
        raise HTTPException(404, "not found")
    if data.what == "all":
        shutil.rmtree(meeting_dir)
    elif data.what == "transcript":
        (meeting_dir / "transcript.md").unlink(missing_ok=True)
    elif data.what == "protocol":
        (meeting_dir / "protocol.md").unlink(missing_ok=True)
    return {"ok": True}
