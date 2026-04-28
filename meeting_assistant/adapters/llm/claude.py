from .base import BaseLLMProvider
from ..config.secrets import get_api_key


class ClaudeProvider(BaseLLMProvider):
    @property
    def label(self) -> str:
        return f"Claude / {self._config.get('protocol', 'claude_model', 'claude-sonnet-4-6')}"

    def _retryable_errors(self) -> tuple:
        import anthropic
        return (anthropic.APIConnectionError, anthropic.APIStatusError)

    def _stream_chunks(self, transcript: str, instructions: str | None):
        import anthropic
        api_key = get_api_key(
            "ANTHROPIC_API_KEY", "anthropic_api_key",
            self._config.get("api", "anthropic_api_key", ""),
        )
        if not api_key:
            raise SystemExit("Ошибка: ANTHROPIC_API_KEY не задан. Запусти setup.sh для инструкции.")
        model = self._config.get("protocol", "claude_model", "claude-sonnet-4-6")
        max_tokens = self._config.get("protocol", "max_tokens", 8192)
        # Transcript as a cached block — regenerating with a different template hits the cache.
        transcript_block = {
            "type": "text",
            "text": transcript,
            "cache_control": {"type": "ephemeral"},
        }
        content = (
            [transcript_block, {"type": "text", "text": instructions}]
            if instructions
            else [transcript_block]
        )
        client = anthropic.Anthropic(api_key=api_key)
        with client.messages.stream(
            model=model,
            max_tokens=max_tokens,
            messages=[{"role": "user", "content": content}],
        ) as stream:
            yield from stream.text_stream
            final = stream.get_final_message()
        if final.stop_reason == "max_tokens":
            print(
                "\n⚠  Протокол обрезан: достигнут лимит токенов. "
                "Увеличь max_tokens в настройках протокола."
            )
