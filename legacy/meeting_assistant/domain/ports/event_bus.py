from abc import ABC, abstractmethod
from typing import Any, Callable


class IEventBus(ABC):
    @abstractmethod
    def publish(self, event: Any) -> None: ...

    @abstractmethod
    def subscribe(self, event_type: type, handler: Callable[[Any], None]) -> None: ...
