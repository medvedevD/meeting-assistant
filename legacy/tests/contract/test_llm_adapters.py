"""
Contract tests for LLM adapters.

Each test mocks the underlying HTTP client (anthropic / openai SDK) at the point
where the adapter imports it, verifying that the adapter:
  1. Calls the client with the correct parameters.
  2. Yields each text chunk.
  3. Returns the fully assembled string from generate().
"""
from pathlib import Path
from unittest.mock import MagicMock, patch, call
import pytest

from tests.unit.fakes import FakeConfigProvider


# ── helpers ────────────────────────────────────────────────────────────────────

def _config(overrides: dict | None = None) -> FakeConfigProvider:
    base = {
        "api": {
            "anthropic_api_key": "test-key",
            "gemini_api_key": "test-gemini-key",
            "mistral_api_key": "test-mistral-key",
        },
        "protocol": {
            "claude_model": "claude-sonnet-4-6",
            "gemini_model": "gemini-2.0-flash-lite",
            "mistral_model": "mistral-small-latest",
            "ollama_model": "qwen2.5:14b",
            "ollama_base_url": "http://localhost:11434/v1",
            "max_tokens": 8192,
        },
    }
    if overrides:
        for section, vals in overrides.items():
            base.setdefault(section, {}).update(vals)
    return FakeConfigProvider(base)


def _partial_path(tmp_path: Path) -> Path:
    return tmp_path / "protocol.md"


# ── ClaudeProvider ─────────────────────────────────────────────────────────────

class TestClaudeProvider:
    def _make_stream_mock(self, chunks: list[str]):
        """Build a mock for anthropic client.messages.stream context manager."""
        stream_cm = MagicMock()
        stream_cm.__enter__ = MagicMock(return_value=stream_cm)
        stream_cm.__exit__ = MagicMock(return_value=False)
        stream_cm.text_stream = iter(chunks)
        final_msg = MagicMock()
        final_msg.stop_reason = "end_turn"
        stream_cm.get_final_message = MagicMock(return_value=final_msg)
        return stream_cm

    def test_returns_assembled_text(self, tmp_path):
        from meeting_assistant.adapters.llm.claude import ClaudeProvider

        stream_mock = self._make_stream_mock(["Hello", " ", "world"])
        client_mock = MagicMock()
        client_mock.messages.stream.return_value = stream_mock

        with patch("meeting_assistant.adapters.llm.claude.get_api_key", return_value="test-key"), \
             patch("anthropic.Anthropic", return_value=client_mock):
            provider = ClaudeProvider(_config())
            result = provider.generate("transcript", "instructions", _partial_path(tmp_path))

        assert result == "Hello world"

    def test_calls_stream_with_correct_model(self, tmp_path):
        from meeting_assistant.adapters.llm.claude import ClaudeProvider

        stream_mock = self._make_stream_mock(["ok"])
        client_mock = MagicMock()
        client_mock.messages.stream.return_value = stream_mock

        with patch("meeting_assistant.adapters.llm.claude.get_api_key", return_value="test-key"), \
             patch("anthropic.Anthropic", return_value=client_mock):
            provider = ClaudeProvider(_config())
            provider.generate("transcript", None, _partial_path(tmp_path))

        call_kwargs = client_mock.messages.stream.call_args.kwargs
        assert call_kwargs["model"] == "claude-sonnet-4-6"
        assert call_kwargs["max_tokens"] == 8192

    def test_transcript_in_content(self, tmp_path):
        from meeting_assistant.adapters.llm.claude import ClaudeProvider

        stream_mock = self._make_stream_mock(["done"])
        client_mock = MagicMock()
        client_mock.messages.stream.return_value = stream_mock

        with patch("meeting_assistant.adapters.llm.claude.get_api_key", return_value="test-key"), \
             patch("anthropic.Anthropic", return_value=client_mock):
            provider = ClaudeProvider(_config())
            provider.generate("MY_TRANSCRIPT", "instructions", _partial_path(tmp_path))

        messages = client_mock.messages.stream.call_args.kwargs["messages"]
        content = messages[0]["content"]
        assert any("MY_TRANSCRIPT" in block.get("text", "") for block in content)

    def test_raises_systemexit_without_api_key(self, tmp_path):
        from meeting_assistant.adapters.llm.claude import ClaudeProvider

        with patch("meeting_assistant.adapters.llm.claude.get_api_key", return_value=""):
            provider = ClaudeProvider(FakeConfigProvider())
            with pytest.raises(SystemExit):
                provider.generate("transcript", None, _partial_path(tmp_path))


# ── OllamaProvider ─────────────────────────────────────────────────────────────

class TestOllamaProvider:
    def _make_stream_chunks(self, texts: list[str]):
        chunks = []
        for t in texts:
            chunk = MagicMock()
            chunk.choices = [MagicMock()]
            chunk.choices[0].delta.content = t
            chunks.append(chunk)
        return chunks

    def test_returns_assembled_text(self, tmp_path):
        from meeting_assistant.adapters.llm.ollama import OllamaProvider

        chunks = self._make_stream_chunks(["Part1", " Part2"])
        completion_mock = MagicMock()
        completion_mock.__enter__ = MagicMock(return_value=iter(chunks))
        completion_mock.__exit__ = MagicMock(return_value=False)
        client_mock = MagicMock()
        client_mock.chat.completions.create.return_value = completion_mock

        with patch("openai.OpenAI", return_value=client_mock):
            provider = OllamaProvider(_config())
            result = provider.generate("transcript", "instructions", _partial_path(tmp_path))

        assert result == "Part1 Part2"

    def test_calls_correct_model_and_base_url(self, tmp_path):
        from meeting_assistant.adapters.llm.ollama import OllamaProvider

        chunks = self._make_stream_chunks(["ok"])
        completion_mock = MagicMock()
        completion_mock.__enter__ = MagicMock(return_value=iter(chunks))
        completion_mock.__exit__ = MagicMock(return_value=False)
        client_mock = MagicMock()
        client_mock.chat.completions.create.return_value = completion_mock

        with patch("openai.OpenAI", return_value=client_mock) as openai_cls:
            provider = OllamaProvider(_config())
            provider.generate("transcript", None, _partial_path(tmp_path))

        init_kwargs = openai_cls.call_args.kwargs
        assert init_kwargs["base_url"] == "http://localhost:11434/v1"

        create_kwargs = client_mock.chat.completions.create.call_args.kwargs
        assert create_kwargs["model"] == "qwen2.5:14b"
        assert create_kwargs["stream"] is True

    def test_prompt_combines_transcript_and_instructions(self, tmp_path):
        from meeting_assistant.adapters.llm.ollama import OllamaProvider

        chunks = self._make_stream_chunks(["ok"])
        completion_mock = MagicMock()
        completion_mock.__enter__ = MagicMock(return_value=iter(chunks))
        completion_mock.__exit__ = MagicMock(return_value=False)
        client_mock = MagicMock()
        client_mock.chat.completions.create.return_value = completion_mock

        with patch("openai.OpenAI", return_value=client_mock):
            provider = OllamaProvider(_config())
            provider.generate("TRANSCRIPT", "INSTRUCTIONS", _partial_path(tmp_path))

        messages = client_mock.chat.completions.create.call_args.kwargs["messages"]
        prompt = messages[0]["content"]
        assert "TRANSCRIPT" in prompt
        assert "INSTRUCTIONS" in prompt


# ── GeminiProvider ─────────────────────────────────────────────────────────────

class TestGeminiProvider:
    def test_returns_assembled_text(self, tmp_path):
        from meeting_assistant.adapters.llm.gemini import GeminiProvider

        chunk1, chunk2 = MagicMock(), MagicMock()
        chunk1.text = "Hello"
        chunk2.text = " Gemini"
        client_mock = MagicMock()
        client_mock.models.generate_content_stream.return_value = iter([chunk1, chunk2])
        genai_mock = MagicMock()
        genai_mock.Client.return_value = client_mock

        # `from google import genai` resolves via the `google` module's attribute,
        # so we need google_mock.genai to point to our genai_mock.
        google_mock = MagicMock()
        google_mock.genai = genai_mock

        with patch("meeting_assistant.adapters.llm.gemini.get_api_key", return_value="test-gemini-key"), \
             patch.dict("sys.modules", {"google": google_mock, "google.genai": genai_mock}):
            provider = GeminiProvider(_config())
            result = provider.generate("transcript", "instructions", _partial_path(tmp_path))

        assert result == "Hello Gemini"

    def test_raises_systemexit_without_api_key(self, tmp_path):
        from meeting_assistant.adapters.llm.gemini import GeminiProvider

        with patch("meeting_assistant.adapters.llm.gemini.get_api_key", return_value=""):
            provider = GeminiProvider(FakeConfigProvider())
            with pytest.raises(SystemExit):
                provider.generate("transcript", None, _partial_path(tmp_path))


# ── MistralProvider ────────────────────────────────────────────────────────────

class TestMistralProvider:
    def test_returns_assembled_text(self, tmp_path):
        from meeting_assistant.adapters.llm.mistral import MistralProvider

        chunk = MagicMock()
        chunk.data.choices[0].delta.content = "Mistral response"
        stream_cm = MagicMock()
        stream_cm.__enter__ = MagicMock(return_value=iter([chunk]))
        stream_cm.__exit__ = MagicMock(return_value=False)
        client_mock = MagicMock()
        client_mock.chat.stream.return_value = stream_cm
        mistral_cls_mock = MagicMock(return_value=client_mock)
        mistral_module_mock = MagicMock()
        mistral_module_mock.Mistral = mistral_cls_mock

        with patch("meeting_assistant.adapters.llm.mistral.get_api_key", return_value="test-mistral-key"), \
             patch.dict("sys.modules", {
                 "mistralai": MagicMock(),
                 "mistralai.client": mistral_module_mock,
             }):
            provider = MistralProvider(_config())
            result = provider.generate("transcript", "instructions", _partial_path(tmp_path))

        assert result == "Mistral response"

    def test_raises_systemexit_without_api_key(self, tmp_path):
        from meeting_assistant.adapters.llm.mistral import MistralProvider

        with patch("meeting_assistant.adapters.llm.mistral.get_api_key", return_value=""):
            provider = MistralProvider(FakeConfigProvider())
            with pytest.raises(SystemExit):
                provider.generate("transcript", None, _partial_path(tmp_path))
