from abc import ABC, abstractmethod


class ITemplateRepository(ABC):
    @abstractmethod
    def list(self) -> list[dict]: ...

    @abstractmethod
    def save(self, name: str, prompt: str) -> None: ...

    @abstractmethod
    def delete(self, name: str) -> None: ...
