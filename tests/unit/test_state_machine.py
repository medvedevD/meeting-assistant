"""Unit tests for MeetingStateMachine — all valid and invalid transitions."""
import pytest

from meeting_assistant.domain.services.state_machine import (
    MeetingStateMachine,
    InvalidTransitionError,
)
from meeting_assistant.domain.value_objects.meeting_status import MeetingStatus

S = MeetingStatus
sm = MeetingStateMachine()


# ── valid transitions ─────────────────────────────────────────────────────────

@pytest.mark.parametrize("from_s, to_s", [
    (S.CREATED,           S.AUDIO_READY),
    (S.AUDIO_READY,       S.TRANSCRIBING),
    (S.TRANSCRIBING,      S.TRANSCRIBED),
    (S.TRANSCRIBING,      S.TRANSCRIBE_FAILED),
    (S.TRANSCRIBING,      S.CANCELLED),
    (S.TRANSCRIBED,       S.GENERATING),
    (S.TRANSCRIBED,       S.COMPLETED),
    (S.TRANSCRIBED,       S.TRANSCRIBING),
    (S.GENERATING,        S.COMPLETED),
    (S.GENERATING,        S.PROTOCOL_FAILED),
    (S.GENERATING,        S.CANCELLED),
    (S.TRANSCRIBE_FAILED, S.TRANSCRIBING),
    (S.PROTOCOL_FAILED,   S.GENERATING),
    (S.COMPLETED,         S.ARCHIVED),
    (S.COMPLETED,         S.TRANSCRIBING),
    (S.COMPLETED,         S.GENERATING),
    (S.CANCELLED,         S.TRANSCRIBING),
    (S.CANCELLED,         S.GENERATING),
])
def test_valid_transition(from_s, to_s):
    sm.validate(from_s, to_s)  # must not raise


# ── invalid transitions ───────────────────────────────────────────────────────

@pytest.mark.parametrize("from_s, to_s", [
    (S.CREATED,    S.TRANSCRIBING),
    (S.CREATED,    S.COMPLETED),
    (S.AUDIO_READY, S.COMPLETED),
    (S.AUDIO_READY, S.TRANSCRIBED),
    (S.TRANSCRIBED, S.AUDIO_READY),
    (S.COMPLETED,  S.CREATED),
    (S.ARCHIVED,   S.COMPLETED),
    (S.ARCHIVED,   S.TRANSCRIBING),
    (S.ARCHIVED,   S.GENERATING),
])
def test_invalid_transition(from_s, to_s):
    with pytest.raises(InvalidTransitionError) as exc_info:
        sm.validate(from_s, to_s)
    assert exc_info.value.from_status == from_s
    assert exc_info.value.to_status == to_s


# ── allowed_from ─────────────────────────────────────────────────────────────

def test_allowed_from_created():
    assert sm.allowed_from(S.CREATED) == frozenset({S.AUDIO_READY})


def test_allowed_from_archived_is_empty():
    assert sm.allowed_from(S.ARCHIVED) == frozenset()


def test_allowed_from_transcribing():
    allowed = sm.allowed_from(S.TRANSCRIBING)
    assert S.TRANSCRIBED in allowed
    assert S.TRANSCRIBE_FAILED in allowed
    assert S.CANCELLED in allowed
    assert S.COMPLETED not in allowed


def test_error_message_contains_status_names():
    with pytest.raises(InvalidTransitionError) as exc_info:
        sm.validate(S.CREATED, S.COMPLETED)
    assert "CREATED" in str(exc_info.value)
    assert "COMPLETED" in str(exc_info.value)
