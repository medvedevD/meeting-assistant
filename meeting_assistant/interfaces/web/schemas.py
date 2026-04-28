from typing import Literal
from pydantic import BaseModel


class RecordStartRequest(BaseModel):
    name: str = ""
    prepend_date: bool = True


class RecordStopRequest(BaseModel):
    name: str = ""


class ProcessStartRequest(BaseModel):
    folder: str
    name: str = ""
    model: str = ""
    no_protocol: bool = False
    from_transcript: bool = False


class MeetingDeleteRequest(BaseModel):
    folder: str
    what: Literal["all", "transcript", "protocol"] = "all"


class TemplateSaveRequest(BaseModel):
    name: str
    prompt: str
    active: str = ""


class TemplateDeleteRequest(BaseModel):
    name: str


class ConfigUpdateRequest(BaseModel):
    transcription: dict | None = None
    recording: dict | None = None
    protocol: dict | None = None
    api: dict | None = None
    storage: dict | None = None
