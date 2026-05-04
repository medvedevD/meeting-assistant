from .base import BaseLLMProvider


class OllamaProvider(BaseLLMProvider):
    @property
    def label(self) -> str:
        return f"Ollama / {self._config.get('protocol', 'ollama_model', 'qwen2.5:14b-instruct-q4_K_M')}"

    def _retryable_errors(self) -> tuple:
        from openai import APIConnectionError, APIStatusError
        return (APIConnectionError, APIStatusError)

    def _stream_chunks(self, transcript: str, instructions: str | None):
        from openai import OpenAI
        base_url = self._config.get("protocol", "ollama_base_url", "http://localhost:11434/v1")
        model = getattr(self, "_model_override", None) or self._config.get("protocol", "ollama_model", "qwen2.5:14b-instruct-q4_K_M")
        prompt = f"{transcript}\n\n{instructions}" if instructions else transcript
        client = OpenAI(api_key="ollama", base_url=base_url)
        with client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": prompt}],
            stream=True,
        ) as stream:
            for chunk in stream:
                text = chunk.choices[0].delta.content
                if text:
                    yield text
