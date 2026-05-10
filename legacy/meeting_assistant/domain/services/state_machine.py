from ..value_objects.meeting_status import MeetingStatus

_ALLOWED: dict[MeetingStatus, frozenset[MeetingStatus]] = {
    MeetingStatus.CREATED:           frozenset({MeetingStatus.AUDIO_READY}),
    MeetingStatus.AUDIO_READY:       frozenset({MeetingStatus.TRANSCRIBING}),
    MeetingStatus.TRANSCRIBING:      frozenset({
        MeetingStatus.TRANSCRIBED,
        MeetingStatus.TRANSCRIBE_FAILED,
        MeetingStatus.CANCELLED,
        MeetingStatus.AUDIO_READY,    # cancel rollback
    }),
    MeetingStatus.TRANSCRIBED:       frozenset({MeetingStatus.GENERATING, MeetingStatus.COMPLETED, MeetingStatus.TRANSCRIBING}),
    MeetingStatus.GENERATING:        frozenset({
        MeetingStatus.COMPLETED,
        MeetingStatus.PROTOCOL_FAILED,
        MeetingStatus.CANCELLED,
        MeetingStatus.TRANSCRIBED,    # cancel rollback
    }),
    MeetingStatus.TRANSCRIBE_FAILED: frozenset({MeetingStatus.TRANSCRIBING}),
    MeetingStatus.PROTOCOL_FAILED:   frozenset({MeetingStatus.GENERATING}),
    MeetingStatus.COMPLETED:         frozenset({
        MeetingStatus.ARCHIVED,
        MeetingStatus.TRANSCRIBING,   # re-transcribe
        MeetingStatus.GENERATING,     # re-generate protocol
    }),
    MeetingStatus.ARCHIVED:          frozenset(),
    MeetingStatus.CANCELLED:         frozenset({MeetingStatus.TRANSCRIBING, MeetingStatus.GENERATING}),
}


class InvalidTransitionError(Exception):
    def __init__(self, from_status: MeetingStatus, to_status: MeetingStatus):
        super().__init__(f"Invalid transition: {from_status.value} → {to_status.value}")
        self.from_status = from_status
        self.to_status = to_status


class MeetingStateMachine:
    def validate(self, from_status: MeetingStatus, to_status: MeetingStatus) -> None:
        if to_status not in _ALLOWED.get(from_status, frozenset()):
            raise InvalidTransitionError(from_status, to_status)

    def allowed_from(self, from_status: MeetingStatus) -> frozenset[MeetingStatus]:
        return _ALLOWED.get(from_status, frozenset())
