from abc import ABC, abstractmethod


class IConfigProvider(ABC):
    @abstractmethod
    def get(self, section: str, key: str, default):
        """Read a single config value."""

    @abstractmethod
    def load(self) -> dict:
        """Return the full merged config dict."""

    @abstractmethod
    def save(self, cfg: dict) -> None:
        """Persist config."""
