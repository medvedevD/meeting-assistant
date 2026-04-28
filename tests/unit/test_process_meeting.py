import pytest
from pathlib import Path

from meeting_assistant.application.process_meeting import ProcessMeetingUseCase, ProcessMeetingRequest
from meeting_assistant.application.events import TranscriptReady, ProtocolGenerated
from .fakes import (
    FakeConfigProvider,
    FakeTranscriber,
    FakeLLMProvider,
    FakeMeetingRepository,
    FakeTemplateRepository,
    FakeEventBus,
)


def _make_use_case(
    config=None,
    transcriber=None,
    llm=None,
    repo=None,
    templates=None,
    bus=None,
):
    return ProcessMeetingUseCase(
        transcriber=transcriber or FakeTranscriber(),
        llm_provider=llm or FakeLLMProvider(),
        meeting_repo=repo or FakeMeetingRepository(),
        template_repo=templates or FakeTemplateRepository(),
        config=config or FakeConfigProvider(),
        event_bus=bus,
    )


class TestProcessMeetingUseCase:
    def test_saves_transcript_and_protocol(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        llm = FakeLLMProvider("Generated protocol text")
        uc = _make_use_case(llm=llm, repo=repo)

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Test Meeting",
            whisper_model="tiny",
        )
        result = uc.execute(req)

        slug = repo.resolve_slug_for_audio(tmp_path / "audio.wav", "Test Meeting")
        assert repo.has_transcript(slug)
        assert repo.has_protocol(slug)
        assert result.protocol_path is not None

    def test_transcriber_called_with_correct_model(self, tmp_path):
        transcriber = FakeTranscriber()
        uc = _make_use_case(transcriber=transcriber, repo=FakeMeetingRepository(tmp_path))

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="base",
        )
        uc.execute(req)

        assert len(transcriber.calls) == 1
        _, model = transcriber.calls[0]
        assert model == "base"

    def test_no_protocol_when_flag_false(self, tmp_path):
        repo = FakeMeetingRepository(tmp_path)
        uc = _make_use_case(repo=repo)

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
            generate_protocol=False,
        )
        result = uc.execute(req)

        slug = repo.resolve_slug_for_audio(tmp_path / "audio.wav", "Meeting")
        assert repo.has_transcript(slug)
        assert not repo.has_protocol(slug)
        assert result.protocol_path is None

    def test_from_transcript_skips_transcriber(self, tmp_path):
        transcriber = FakeTranscriber()
        repo = FakeMeetingRepository(tmp_path)
        slug = repo.resolve_slug_for_audio(tmp_path / "audio.wav", "Meeting")
        repo.save_transcript(slug, "Meeting", "[00:00] Existing transcript", "01.01.2024")

        uc = _make_use_case(transcriber=transcriber, repo=repo)
        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
            from_transcript=True,
        )
        uc.execute(req)

        assert len(transcriber.calls) == 0

    def test_publishes_transcript_ready_event(self, tmp_path):
        bus = FakeEventBus()
        uc = _make_use_case(repo=FakeMeetingRepository(tmp_path), bus=bus)

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
        )
        uc.execute(req)

        events = bus.events_of(TranscriptReady)
        assert len(events) == 1

    def test_publishes_protocol_generated_event(self, tmp_path):
        bus = FakeEventBus()
        uc = _make_use_case(repo=FakeMeetingRepository(tmp_path), bus=bus)

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
        )
        uc.execute(req)

        events = bus.events_of(ProtocolGenerated)
        assert len(events) == 1

    def test_no_events_published_without_bus(self, tmp_path):
        uc = _make_use_case(repo=FakeMeetingRepository(tmp_path), bus=None)

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
        )
        # should not raise
        uc.execute(req)

    def test_template_instructions_passed_to_llm(self, tmp_path):
        llm = FakeLLMProvider()
        templates = FakeTemplateRepository([
            {"name": "Daily", "prompt": "Summarize: {transcript} for {meeting_name}"}
        ])
        config = FakeConfigProvider({"protocol": {"active_template": "Daily"}})
        uc = _make_use_case(llm=llm, templates=templates, config=config, repo=FakeMeetingRepository(tmp_path))

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Standup",
            whisper_model="tiny",
        )
        uc.execute(req)

        _, instructions = llm.calls[0]
        assert instructions is not None
        assert "Standup" in instructions

    def test_no_active_template_passes_none_instructions(self, tmp_path):
        llm = FakeLLMProvider()
        uc = _make_use_case(llm=llm, repo=FakeMeetingRepository(tmp_path))

        req = ProcessMeetingRequest(
            audio_path=tmp_path / "audio.wav",
            meeting_name="Meeting",
            whisper_model="tiny",
        )
        uc.execute(req)

        _, instructions = llm.calls[0]
        assert instructions is None
