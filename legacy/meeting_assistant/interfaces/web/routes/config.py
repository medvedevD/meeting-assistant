import json
import logging
import os
import subprocess
import urllib.request
from pathlib import Path
from fastapi import APIRouter, Request, Query, HTTPException
from ..schemas import ConfigUpdateRequest
from ....adapters.config.secrets import get_api_key_info, set_api_key, delete_api_key

router = APIRouter()
_log = logging.getLogger(__name__)

_API_KEYS = [
    # (config_field_name, env_var, keyring_key)
    ("anthropic_api_key", "ANTHROPIC_API_KEY", "anthropic_api_key"),
    ("gemini_api_key",    "GEMINI_API_KEY",    "gemini_api_key"),
    ("mistral_api_key",   "MISTRAL_API_KEY",   "mistral_api_key"),
]

_VALID_KEY_NAMES = {field for field, _, _ in _API_KEYS}


@router.get("/config")
def get_config(request: Request):
    cfg = request.app.state.container.config.load()
    api = {}
    for field, env_var, kr_key in _API_KEYS:
        api[field] = get_api_key_info(env_var, kr_key)
    cfg["api"] = api
    return cfg


@router.post("/config")
def post_config(data: ConfigUpdateRequest, request: Request):
    config = request.app.state.container.config
    cfg = config.load()
    for section in ("transcription", "recording", "protocol"):
        val = getattr(data, section, None)
        if val:
            cfg[section].update(val)
    if data.api:
        for field, env_var, kr_key in _API_KEYS:
            key = data.api.get(field, "")
            if key and key != "***":
                set_api_key(kr_key, key)
                cfg.setdefault("api", {}).pop(field, None)
    if data.storage:
        new_dir = data.storage.get("meetings_dir", "")
        if new_dir:
            resolved = Path(new_dir).expanduser()
            try:
                resolved.mkdir(parents=True, exist_ok=True, mode=0o700)
            except OSError as exc:
                raise HTTPException(status_code=400, detail=str(exc))
            cfg.setdefault("storage", {})["meetings_dir"] = new_dir
            request.app.state.container.rebuild_meeting_repo(new_dir)
    config.save(cfg)
    return {"ok": True}


@router.get("/storage/validate")
def validate_storage_path(path: str = Query(...), request: Request = None):
    if not path:
        return {"error": "путь не указан"}
    resolved = Path(path).expanduser().resolve()
    exists = resolved.exists()
    writable = False
    error = None
    if exists:
        writable = os.access(resolved, os.W_OK)
        if not writable:
            error = "недоступен на запись"
    else:
        # check parent writability
        parent = resolved
        while not parent.exists():
            parent = parent.parent
        writable = os.access(parent, os.W_OK)
        if not writable:
            error = "родительский каталог недоступен на запись"
    return {"resolved": str(resolved), "exists": exists, "writable": writable, "error": error}


@router.delete("/config/api_key/{name}")
def delete_key(name: str):
    if name not in _VALID_KEY_NAMES:
        raise HTTPException(status_code=404, detail=f"Unknown key: {name}")
    ok = delete_api_key(name)
    if not ok:
        raise HTTPException(status_code=500, detail="keyring unavailable or key not found")
    return {"ok": True}


@router.post("/config/api_key/{name}/test")
def test_key(name: str, request: Request):
    if name not in _VALID_KEY_NAMES:
        raise HTTPException(status_code=404, detail=f"Unknown key: {name}")

    entry = next((e for e in _API_KEYS if e[0] == name), None)
    _, env_var, kr_key = entry

    from ....adapters.config.secrets import get_api_key
    key = get_api_key(env_var, kr_key)
    if not key:
        return {"ok": False, "error": "ключ не задан"}

    cfg = request.app.state.container.config.load()
    proto = cfg.get("protocol", {})
    provider = name.replace("_api_key", "")   # anthropic / gemini / mistral
    try:
        if provider == "anthropic":
            import anthropic
            model = proto.get("claude_model", "claude-haiku-4-5-20251001")
            client = anthropic.Anthropic(api_key=key)
            # count_tokens is free — validates key AND model access without spending tokens
            client.messages.count_tokens(
                model=model,
                messages=[{"role": "user", "content": "hi"}],
            )
        elif provider == "gemini":
            from google import genai
            model = proto.get("gemini_model", "gemini-2.0-flash-lite")
            client = genai.Client(api_key=key)
            # count_tokens uses a separate quota from generate_content — catches billing issues
            client.models.count_tokens(model=model, contents="hi")
        elif provider == "mistral":
            from mistralai.client import Mistral
            model = proto.get("mistral_model", "mistral-small-latest")
            client = Mistral(api_key=key)
            # No free count_tokens in this SDK; verify key + model availability
            available = [m.id for m in client.models.list().data]
            if model not in available:
                return {"ok": False, "error": f"модель {model!r} недоступна (доступны: {', '.join(available[:5])})"}
        return {"ok": True}
    except Exception as exc:
        _log.warning("Key test failed for %s: %s", provider, exc)
        return {"ok": False, "error": str(exc)}


@router.get("/sources")
def get_sources():
    try:
        src_out = subprocess.check_output(["pactl", "list", "sources", "short"], text=True)
        snk_out = subprocess.check_output(["pactl", "list", "sinks", "short"], text=True)
    except Exception:
        return {"mics": [], "sinks": []}
    mics = [
        p[1] for line in src_out.splitlines()
        if len(p := line.split("\t")) >= 2 and not p[1].endswith(".monitor")
    ]
    sinks = [
        p[1] + ".monitor" for line in snk_out.splitlines()
        if len(p := line.split("\t")) >= 2
    ]
    return {"mics": mics, "sinks": sinks}


@router.get("/models")
def get_models():
    from gui_config import load_models
    return load_models()


@router.get("/ollama/status")
def get_ollama_status(base_url: str = Query(default="http://localhost:11434/v1")):
    base_url = base_url.rstrip("/")
    try:
        req = urllib.request.Request(
            f"{base_url}/models", headers={"Accept": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=3) as resp:
            data = json.loads(resp.read())
        models = [m["id"] for m in data.get("data", [])]
        return {"ok": True, "models": models}
    except Exception as exc:
        return {"ok": False, "error": str(exc)}
