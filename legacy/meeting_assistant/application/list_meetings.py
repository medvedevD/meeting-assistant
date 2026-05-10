from ..domain.ports.meeting_repository import IMeetingRepository
from ..domain.entities.meeting import Meeting


class ListMeetingsUseCase:
    def __init__(self, meeting_repo: IMeetingRepository):
        self._repo = meeting_repo

    def execute(self) -> list[Meeting]:
        return self._repo.list()
