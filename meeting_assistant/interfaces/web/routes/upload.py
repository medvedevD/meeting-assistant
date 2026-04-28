from fastapi import APIRouter, Request, HTTPException, UploadFile, Form

from ..schemas import ProcessStartRequest
from ..sse import ProcessingEvent
from .processing import start_processing_job
from ....application.upload_audio import UploadAudioUseCase

router = APIRouter()

_ALLOWED_MIME_PREFIXES = ("audio/", "video/webm", "application/octet-stream")
_MAX_SIZE_BYTES = 4 * 1024 * 1024 * 1024  # 4 GB hard limit


@router.post("/upload")
async def post_upload(
    request: Request,
    file: UploadFile,
    title: str = Form(default=""),
    auto_process: bool = Form(default=True),
    no_protocol: bool = Form(default=False),
):
    content_type = (file.content_type or "").lower()
    if not any(content_type.startswith(p) for p in _ALLOWED_MIME_PREFIXES):
        raise HTTPException(415, f"Неподдерживаемый тип файла: {content_type}. Ожидается аудиофайл.")

    container = request.app.state.container
    repo = container.meeting_repo
    use_case = UploadAudioUseCase(repo)

    result = use_case.execute(
        title=title,
        upload_stream=file.file,
        original_filename=file.filename or "upload",
    )

    if auto_process:
        ws = request.app.state.web_state
        proc_data = ProcessStartRequest(
            folder=result.folder,
            name=title,
            no_protocol=no_protocol,
        )
        start_processing_job(ws, container, str(result.audio_path), proc_data)

    return {"slug": result.slug, "folder": result.folder, "audio_path": str(result.audio_path)}
