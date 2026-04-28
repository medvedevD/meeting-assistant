from ..domain.ports.meeting_repository import IMeetingRepository
from ..domain.ports.event_bus import IEventBus
from ..domain.value_objects.meeting_slug import MeetingSlug
from .events import MeetingDeleted


class DeleteMeetingUseCase:
    def __init__(self, meeting_repo: IMeetingRepository, event_bus: IEventBus | None = None):
        self._repo = meeting_repo
        self._bus = event_bus

    def execute(self, slug: MeetingSlug) -> None:
        self._repo.delete(slug)
        if self._bus:
            self._bus.publish(MeetingDeleted(slug=slug))
