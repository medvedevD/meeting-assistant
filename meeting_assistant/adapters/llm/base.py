import logging
import time
from abc import ABC, abstractmethod
from pathlib import Path

from ...domain.ports.config_provider import IConfigProvider

_log = logging.getLogger(__name__)


class BaseLLMProvider(ABC):
    RETRY_WAITS = [5, 20]

    def __init__(self, config: IConfigProvider):
        self._config = config

    @property
    @abstractmethod
    def label(self) -> str: ...

    @abstractmethod
    def _retryable_errors(self) -> tuple: ...

    @abstractmethod
    def _stream_chunks(self, transcript: str, instructions: str | None): ...

    def generate(self, transcript: str, instructions: str | None, partial_save_path: Path) -> str:
        max_retries = len(self.RETRY_WAITS) + 1
        for attempt in range(max_retries):
            result: list[str] = []
            try:
                suffix = f" (попытка {attempt + 1}/{max_retries})" if attempt > 0 else ""
                _log.info("Генерирую протокол (%s)%s", self.label, suffix)
                for chunk in self._stream_chunks(transcript, instructions):
                    result.append(chunk)
                _log.info("Генерация завершена (%d символов)", sum(len(c) for c in result))
                return "".join(result)
            except self._retryable_errors() as exc:
                if attempt < max_retries - 1:
                    wait = self.RETRY_WAITS[attempt]
                    _log.warning("Ошибка API: %s. Повтор через %dс...", exc, wait)
                    time.sleep(wait)
                else:
                    _log.error("Ошибка после %d попыток: %s", max_retries, exc)
                    partial = "".join(result)
                    if partial:
                        p = partial_save_path.with_name(partial_save_path.stem + ".partial.md")
                        p.write_text(partial, encoding="utf-8")
                        _log.info("Частичный ответ сохранён: %s", p)
                    raise
