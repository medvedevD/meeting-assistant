"""Чтение и запись config.json для GUI и скриптов обработки."""

import json
import re
from pathlib import Path

ROOT_DIR = Path(__file__).parent.parent
PROMPTS_DIR = ROOT_DIR / "prompts"


def _resolve_config_path() -> Path:
    import os
    if env := os.environ.get("MEETING_ASSISTANT_CONFIG"):
        return Path(env).expanduser()
    xdg = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(xdg) / "meeting-assistant" / "config.json"


CONFIG_PATH = _resolve_config_path()


DEFAULTS: dict = {
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


def load() -> dict:
    if not CONFIG_PATH.exists():
        return json.loads(json.dumps(DEFAULTS))
    with open(CONFIG_PATH, encoding="utf-8") as f:
        saved = json.load(f)
    merged = json.loads(json.dumps(DEFAULTS))
    for key, value in saved.items():
        # skip legacy templates list stored in config
        if key == "templates":
            continue
        if key in merged and isinstance(value, dict) and isinstance(merged[key], dict):
            merged[key].update(value)
        else:
            merged[key] = value
    # migrate: old configs without kind — treat as cloud
    if "kind" not in merged.get("protocol", {}):
        merged.setdefault("protocol", {})["kind"] = "cloud"
    return merged


def save(cfg: dict) -> None:
    # never persist templates into config.json — they live in prompts/
    cfg.pop("templates", None)
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)


def _name_to_file(name: str) -> Path:
    safe = re.sub(r"[/\\\0]", "_", name)
    return PROMPTS_DIR / f"{safe}.md"


CLOUD_PROVIDER_MODELS: dict[str, list[str]] = {
    "claude": [
        "claude-opus-4-7",
        "claude-sonnet-4-6",
        "claude-haiku-4-5-20251001",
    ],
    "gemini": [
        "gemini-2.0-flash",
        "gemini-2.0-flash-lite",
        "gemini-1.5-pro",
        "gemini-1.5-flash",
    ],
    "mistral": [
        "mistral-large-latest",
        "mistral-medium-latest",
        "mistral-small-latest",
        "open-mistral-nemo",
    ],
}

LOCAL_PROVIDER_MODELS: dict[str, list[str]] = {
    "ollama": [
        "qwen2.5:14b-instruct-q4_K_M",
        "qwen2.5:7b-instruct-q4_K_M",
        "mistral-nemo:12b-instruct-q4_K_M",
        "gemma2:9b-instruct-q4_K_M",
        "vikhr-nemo-12b-instruct:latest",
        "saiga-llama3-8b:latest",
    ],
}


def load_models() -> dict:
    return {"cloud": CLOUD_PROVIDER_MODELS, "local": LOCAL_PROVIDER_MODELS}


def load_templates() -> list[dict]:
    PROMPTS_DIR.mkdir(exist_ok=True)
    files = sorted(PROMPTS_DIR.glob("*.md"))
    return [{"name": f.stem, "prompt": f.read_text(encoding="utf-8")} for f in files]


def save_template(name: str, prompt: str) -> None:
    PROMPTS_DIR.mkdir(exist_ok=True)
    _name_to_file(name).write_text(prompt, encoding="utf-8")


def delete_template(name: str) -> None:
    f = _name_to_file(name)
    if f.exists():
        f.unlink()
