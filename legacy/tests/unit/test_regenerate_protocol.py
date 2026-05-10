from meeting_assistant.application.regenerate_protocol import (
    RegenerateProtocolUseCase,
    RegenerateProtocolRequest,
)
from meeting_assistant.application.events import ProtocolGenerated
from meeting_assistant.domain.value_objects.meeting_slug import MeetingSlug
from .fakes import (
    FakeConfigProvider,
    FakeLLMProvider,
    FakeMeetingRepository,
    FakeTemplateRepository,
    FakeEventBus,
)


def _seed_transcript(repo: FakeMeetingRepository, slug: MeetingSlug, text: str) -> None:
    repo.save_transcript(slug, slug.value, text, "01.01.2024")


class TestRegenerateProtocolUseCase:
    def test_generates_and_saves_protocol(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_meeting")
        _seed_transcript(repo, slug, "[00:00] Discussion text")
        llm = FakeLLMProvider("Regenerated protocol")

        uc = RegenerateProtocolUseCase(
            llm_provider=llm,
            meeting_repo=repo,
            template_repo=FakeTemplateRepository(),
            config=FakeConfigProvider(),
        )
        result = uc.execute(RegenerateProtocolRequest(slug=slug, meeting_name="Meeting"))

        assert repo.has_protocol(slug)
        assert result.protocol_path.name == "protocol.md"

    def test_transcript_text_passed_to_llm(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_meeting")
        _seed_transcript(repo, slug, "[00:00] Some content")
        llm = FakeLLMProvider()

        uc = RegenerateProtocolUseCase(
            llm_provider=llm,
            meeting_repo=repo,
            template_repo=FakeTemplateRepository(),
            config=FakeConfigProvider(),
        )
        uc.execute(RegenerateProtocolRequest(slug=slug, meeting_name="Meeting"))

        transcript, _ = llm.calls[0]
        assert "Some content" in transcript

    def test_publishes_protocol_generated_event(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_meeting")
        _seed_transcript(repo, slug, "[00:00] Text")
        bus = FakeEventBus()

        uc = RegenerateProtocolUseCase(
            llm_provider=FakeLLMProvider(),
            meeting_repo=repo,
            template_repo=FakeTemplateRepository(),
            config=FakeConfigProvider(),
            event_bus=bus,
        )
        uc.execute(RegenerateProtocolRequest(slug=slug, meeting_name="Meeting"))

        events = bus.events_of(ProtocolGenerated)
        assert len(events) == 1
        assert events[0].slug == slug

    def test_no_event_without_bus(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_meeting")
        _seed_transcript(repo, slug, "[00:00] Text")

        uc = RegenerateProtocolUseCase(
            llm_provider=FakeLLMProvider(),
            meeting_repo=repo,
            template_repo=FakeTemplateRepository(),
            config=FakeConfigProvider(),
        )
        # should not raise
        uc.execute(RegenerateProtocolRequest(slug=slug, meeting_name="Meeting"))

    def test_active_template_instructions_applied(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        slug = MeetingSlug("2024-01-01_meeting")
        _seed_transcript(repo, slug, "[00:00] Text")
        llm = FakeLLMProvider()
        templates = FakeTemplateRepository([
            {"name": "1on1", "prompt": "Instructions for {meeting_name}: {transcript}"}
        ])
        config = FakeConfigProvider({"protocol": {"active_template": "1on1"}})

        uc = RegenerateProtocolUseCase(
            llm_provider=llm,
            meeting_repo=repo,
            template_repo=templates,
            config=config,
        )
        uc.execute(RegenerateProtocolRequest(slug=slug, meeting_name="My Meeting"))

        _, instructions = llm.calls[0]
        assert instructions is not None
        assert "My Meeting" in instructions
