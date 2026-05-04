from .base import BaseLLMProvider
from ..config.secrets import get_api_key


class MistralProvider(BaseLLMProvider):
    @property
    def label(self) -> str:
        return f"Mistral / {self._config.get('protocol', 'mistral_model', 'mistral-small-latest')}"

    def _retryable_errors(self) -> tuple:
        return (Exception,)

    def _stream_chunks(self, transcript: str, instructions: str | None):
        from mistralai.client import Mistral
        api_key = get_api_key(
            "MISTRAL_API_KEY", "mistral_api_key",
            self._config.get("api", "mistral_api_key", ""),
        )
        if not api_key:
            raise SystemExit(
                "Ошибка: MISTRAL_API_KEY не задан. Укажи ключ в настройках или переменной окружения."
            )
        model = getattr(self, "_model_override", None) or self._config.get("protocol", "mistral_model", "mistral-small-latest")
        prompt = f"{transcript}\n\n{instructions}" if instructions else transcript
        client = Mistral(api_key=api_key)
        with client.chat.stream(
            model=model,
            messages=[{"role": "user", "content": prompt}],
        ) as stream:
            for chunk in stream:
                text = chunk.data.choices[0].delta.content
                if text:
                    yield text
