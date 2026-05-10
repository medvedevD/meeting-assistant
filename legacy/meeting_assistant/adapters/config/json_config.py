import json
import logging
from pathlib import Path
from ...domain.ports.config_provider import IConfigProvider

_log = logging.getLogger(__name__)

# Keys that must never be written to disk — live in keyring/env only.
_SECRET_KEYS = ("anthropic_api_key", "gemini_api_key", "mistral_api_key")

_DEFAULTS: dict = {
    "transcription": {
        "model": "medium",
        "language": "ru",
        "vad_filter": True,
        "min_silence_duration_ms": 500,
        "speech_pad_ms": 200,
        "beam_size": 5,
        "device": "cpu",
        "compute_type": "int8",
    },
    "recording": {
        "mic_source": "",
        "system_source": "",
        "prepend_date": True,
    },
    "protocol": {
        "kind": "cloud",
        "provider": "claude",
        "claude_model": "claude-sonnet-4-6",
        "gemini_model": "gemini-2.0-flash-lite",
        "mistral_model": "mistral-small-latest",
        "ollama_model": "qwen2.5:14b-instruct-q4_K_M",
        "ollama_base_url": "http://localhost:11434/v1",
        "max_tokens": 8192,
        "active_template": "1-на-1",
    },
    "api": {
        "anthropic_api_key": "",
        "gemini_api_key": "",
        "mistral_api_key": "",
    },
}


class JsonConfigProvider(IConfigProvider):
    def __init__(self, config_path: Path):
        self._path = config_path

    def get(self, section: str, key: str, default):
        return self.load().get(section, {}).get(key, default)

    def load(self) -> dict:
        merged = json.loads(json.dumps(_DEFAULTS))
        if not self._path.exists():
            return merged
        with open(self._path, encoding="utf-8") as f:
            saved = json.load(f)
        for key, value in saved.items():
            if key == "templates":
                continue
            if key in merged and isinstance(value, dict) and isinstance(merged[key], dict):
                merged[key].update(value)
            else:
                merged[key] = value
        merged.setdefault("protocol", {}).setdefault("kind", "cloud")
        self._migrate_legacy_secrets(saved, merged)
        return merged

    def _migrate_legacy_secrets(self, saved: dict, merged: dict) -> None:
        """One-time migration: move plaintext API keys from config.json to keyring."""
        from ..config.secrets import set_api_key
        api_section = saved.get("api", {})
        migrated = False
        for key in _SECRET_KEYS:
            val = api_section.get(key, "")
            if val:
                ok = set_api_key(key, val)
                if ok:
                    _log.info("Migrated %s from config.json to keyring", key)
                else:
                    _log.warning("keyring unavailable; %s stays as env-var fallback", key)
                    continue
                merged.setdefault("api", {}).pop(key, None)
                migrated = True
        if migrated:
            # Rewrite file without the secrets so they don't linger on disk.
            self.save(merged)

    def save(self, cfg: dict) -> None:
        cfg.pop("templates", None)
        # Strip all secrets — they belong in keyring/env, never on disk.
        api = cfg.get("api", {})
        for key in _SECRET_KEYS:
            api.pop(key, None)
        if not api:
            cfg.pop("api", None)
        with open(self._path, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
