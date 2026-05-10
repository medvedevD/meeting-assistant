from pathlib import Path

from ...domain.ports.llm_provider import ILLMProvider
from ...domain.ports.config_provider import IConfigProvider
from .claude import ClaudeProvider
from .gemini import GeminiProvider
from .mistral import MistralProvider
from .ollama import OllamaProvider


class LLMProviderAdapter(ILLMProvider):
    def __init__(self, config: IConfigProvider):
        self._config = config

    def generate(self, transcript: str, instructions: str | None, partial_save_path: Path, *, model: str | None = None, provider: str | None = None) -> str:
        kind = self._config.get("protocol", "kind", "cloud")
        provider_name = provider or self._config.get("protocol", "provider", "claude")
        if kind == "local" and not provider:
            cls = OllamaProvider
        else:
            cls = {
                "claude": ClaudeProvider,
                "gemini": GeminiProvider,
                "mistral": MistralProvider,
                "ollama": OllamaProvider,
            }.get(provider_name, ClaudeProvider)
        return cls(self._config).generate(transcript, instructions, partial_save_path, model=model)
