from meeting_assistant.application.delete_meeting import DeleteMeetingUseCase
from meeting_assistant.application.events import MeetingDeleted
from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
from .fakes import FakeMeetingRepository, FakeEventBus


class TestDeleteMeetingUseCase:
    def test_deletes_meeting_from_repository(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_standup")
        repo.save_transcript(slug, "standup", "[00:00] text", "01.01.2024")
        assert len(repo.list()) == 1

        uc = DeleteMeetingUseCase(meeting_repo=repo)
        uc.execute(slug)

        assert len(repo.list()) == 0

    def test_publishes_meeting_deleted_event(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_standup")
        repo.save_transcript(slug, "standup", "[00:00] text", "01.01.2024")
        bus = FakeEventBus()

        uc = DeleteMeetingUseCase(meeting_repo=repo, event_bus=bus)
        uc.execute(slug)

        events = bus.events_of(MeetingDeleted)
        assert len(events) == 1
        assert events[0].slug == slug

    def test_no_error_without_event_bus(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_standup")
        repo.save_transcript(slug, "standup", "[00:00] text", "01.01.2024")

        uc = DeleteMeetingUseCase(meeting_repo=repo, event_bus=None)
        uc.execute(slug)  # must not raise

        assert len(repo.list()) == 0

    def test_event_slug_matches_deleted_slug(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_review")
        repo.save_transcript(slug, "review", "[00:00] text", "01.01.2024")
        bus = FakeEventBus()

        uc = DeleteMeetingUseCase(meeting_repo=repo, event_bus=bus)
        uc.execute(slug)

        event = bus.events_of(MeetingDeleted)[0]
        assert event.slug.value == "2024-01-01_review"
