from .base import BaseLLMProvider
from ..config.secrets import get_api_key


class GeminiProvider(BaseLLMProvider):
    @property
    def label(self) -> str:
        return f"Gemini / {self._config.get('protocol', 'gemini_model', 'gemini-2.0-flash-lite')}"

    def _retryable_errors(self) -> tuple:
        return (Exception,)

    def _stream_chunks(self, transcript: str, instructions: str | None):
        from google import genai
        api_key = get_api_key(
            "GEMINI_API_KEY", "gemini_api_key",
            self._config.get("api", "gemini_api_key", ""),
        )
        if not api_key:
            raise SystemExit(
                "Ошибка: GEMINI_API_KEY не задан. Укажи ключ в настройках или переменной окружения."
            )
        model = getattr(self, "_model_override", None) or self._config.get("protocol", "gemini_model", "gemini-2.0-flash-lite")
        prompt = f"{transcript}\n\n{instructions}" if instructions else transcript
        client = genai.Client(api_key=api_key)
        for chunk in client.models.generate_content_stream(model=model, contents=prompt):
            if chunk.text:
                yield chunk.text
