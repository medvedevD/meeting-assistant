import logging
from collections import defaultdict
from typing import Any, Callable

from ...domain.ports.event_bus import IEventBus

_log = logging.getLogger(__name__)


class InProcessEventBus(IEventBus):
    def __init__(self):
        self._handlers: dict[type, list[Callable]] = defaultdict(list)

    def subscribe(self, event_type: type, handler: Callable[[Any], None]) -> None:
        self._handlers[event_type].append(handler)

    def publish(self, event: Any) -> None:
        for handler in self._handlers[type(event)]:
            try:
                handler(event)
            except Exception as exc:
                _log.warning("EventBus handler error (%s): %s", type(event).__name__, exc)
