import re
from pathlib import Path
from fastapi import APIRouter, Request, HTTPException, Query
from ..schemas import MeetingDeleteRequest
from ....domain.value_objects.meeting_slug import MeetingSlug

router = APIRouter()

_FOLDER_RE = re.compile(r"^[\w\-\. ]+$")


def _validate_folder(folder: str) -> str:
    if not _FOLDER_RE.match(folder):
        raise HTTPException(400, "invalid folder")
    return folder


def _resolve_meeting_dir(folder: str, meetings_dir: Path) -> Path:
    _validate_folder(folder)
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
    repo = request.app.state.container.meeting_repo
    meetings = repo.list()
    folders = [
        {
            "slug": m.slug.value,
            "has_transcript": m.has_transcript,
            "has_protocol": m.has_protocol,
            "status": m.status.value,
        }
        for m in meetings
    ]
    return {"folders": folders}


@router.get("/meeting")
def get_meeting(folder: str = Query(), request: Request = None):
    repo = request.app.state.container.meeting_repo
    meetings_dir = repo.meetings_dir
    meeting_dir = _resolve_meeting_dir(folder, meetings_dir)
    protocol_path = meeting_dir / "protocol.md"
    transcript_path = meeting_dir / "transcript.md"
    slug = MeetingSlug(folder)
    meeting = repo.get(slug)
    status = meeting.status.value if meeting else "COMPLETED"

    error_kind = None
    error_message = None
    current_job_id = None
    job_repo = request.app.state.container.job_repo
    if job_repo:
        if status.endswith("_FAILED"):
            kind = "transcribe" if status == "TRANSCRIBE_FAILED" else "protocol"
            last_job = job_repo.find_latest_for_slug(folder, kind)
            if last_job and last_job.status == "failed":
                error_kind = last_job.error_kind
                error_message = last_job.error_message
        elif status in ("TRANSCRIBING", "GENERATING"):
            kind = "transcribe" if status == "TRANSCRIBING" else "protocol"
            active_job = job_repo.find_latest_for_slug(folder, kind)
            if active_job and active_job.status in ("running", "queued"):
                current_job_id = active_job.id

    return {
        "protocol": protocol_path.read_text(encoding="utf-8") if protocol_path.exists() else None,
        "transcript": transcript_path.read_text(encoding="utf-8") if transcript_path.exists() else None,
        "status": status,
        "error_kind": error_kind,
        "error_message": error_message,
        "current_job_id": current_job_id,
        "transcription_model": meeting.transcription_model if meeting else None,
        "protocol_model": meeting.protocol_model if meeting else None,
        "template_name": meeting.template_name if meeting else None,
    }


@router.post("/meeting/delete")
def delete_meeting(data: MeetingDeleteRequest, request: Request):
    container = request.app.state.container
    repo = container.meeting_repo
    _validate_folder(data.folder)
    slug = MeetingSlug(data.folder)
    meeting_dir = repo.get_meeting_dir(slug)

    if data.what == "all":
        # OK to delete even if folder is gone — clean up DB entry regardless
        if not meeting_dir.exists() and repo.get(slug) is None:
            raise HTTPException(404, "not found")
        container.delete_meeting.execute(slug)
    else:
        if not meeting_dir.exists():
            raise HTTPException(404, "not found")
        if data.what == "transcript":
            repo.delete_transcript(slug)
        elif data.what == "protocol":
            repo.delete_protocol(slug)
    return {"ok": True}


@router.delete("/meeting/audio")
def delete_meeting_audio(folder: str = Query(), request: Request = None):
    container = request.app.state.container
    repo = container.meeting_repo
    _validate_folder(folder)
    slug = MeetingSlug(folder)
    meeting_dir = repo.get_meeting_dir(slug)
    if not meeting_dir.exists():
        raise HTTPException(404, "not found")
    deleted = [p.name for p in meeting_dir.glob("recording.*")]
    repo.delete_audio(slug)
    return {"ok": True, "deleted": deleted}
