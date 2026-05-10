from dataclasses import dataclass

from meeting_assistant.adapters.event_bus.in_process import InProcessEventBus


@dataclass
class EventA:
    value: int


@dataclass
class EventB:
    value: str


class TestInProcessEventBus:
    def test_handler_called_for_subscribed_event(self):
        bus = InProcessEventBus()
        received = []
        bus.subscribe(EventA, received.append)

        bus.publish(EventA(value=42))

        assert len(received) == 1
        assert received[0].value == 42

    def test_handler_not_called_for_other_event_type(self):
        bus = InProcessEventBus()
        received = []
        bus.subscribe(EventA, received.append)

        bus.publish(EventB(value="hello"))

        assert received == []

    def test_multiple_handlers_all_called(self):
        bus = InProcessEventBus()
        log: list[str] = []
        bus.subscribe(EventA, lambda e: log.append(f"h1:{e.value}"))
        bus.subscribe(EventA, lambda e: log.append(f"h2:{e.value}"))

        bus.publish(EventA(value=1))

        assert "h1:1" in log
        assert "h2:1" in log

    def test_failing_handler_does_not_stop_other_handlers(self):
        bus = InProcessEventBus()
        log: list[int] = []

        def bad_handler(e):
            raise RuntimeError("boom")

        bus.subscribe(EventA, bad_handler)
        bus.subscribe(EventA, lambda e: log.append(e.value))

        bus.publish(EventA(value=7))  # must not raise

        assert log == [7]

    def test_multiple_events_published_independently(self):
        bus = InProcessEventBus()
        a_log, b_log = [], []
        bus.subscribe(EventA, a_log.append)
        bus.subscribe(EventB, b_log.append)

        bus.publish(EventA(value=1))
        bus.publish(EventB(value="x"))
        bus.publish(EventA(value=2))

        assert len(a_log) == 2
        assert len(b_log) == 1

    def test_no_handlers_publish_is_silent(self):
        bus = InProcessEventBus()
        bus.publish(EventA(value=0))  # no handlers registered — should not raise

    def test_subscribe_same_handler_twice_calls_twice(self):
        bus = InProcessEventBus()
        log = []
        handler = lambda e: log.append(e.value)  # noqa: E731
        bus.subscribe(EventA, handler)
        bus.subscribe(EventA, handler)

        bus.publish(EventA(value=5))

        assert log == [5, 5]
