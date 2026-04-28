"""
Lookup chain for API keys: env var → keyring → fallback value.
keyring is optional — if not installed or no backend available, silently skipped.
"""
import os

_KEYRING_SERVICE = "meeting-assistant"


def _mask(value: str) -> str:
    """Return first 7 + *** + last 4 chars, or empty string if too short."""
    if len(value) < 12:
        return "***"
    return value[:7] + "***" + value[-4:]


def _keyring_get(keyring_key: str) -> str:
    try:
        import keyring
        return keyring.get_password(_KEYRING_SERVICE, keyring_key) or ""
    except Exception:
        return ""


def get_api_key(env_var: str, keyring_key: str, fallback: str = "") -> str:
    val = os.environ.get(env_var, "")
    if val:
        return val
    val = _keyring_get(keyring_key)
    if val:
        return val
    return fallback


def get_api_key_info(env_var: str, keyring_key: str) -> dict:
    """Return source, masked value, and shadow-warning for a key.

    Returns:
        {
            "source": "env" | "keyring" | "none",
            "masked": "sk-ant-***wXyZ" | "",
            "env_shadows_keyring": bool  # True when env overrides a keyring entry
        }
    """
    env_val = os.environ.get(env_var, "")
    kr_val = _keyring_get(keyring_key)

    if env_val:
        return {
            "source": "env",
            "masked": _mask(env_val),
            "env_shadows_keyring": bool(kr_val),
        }
    if kr_val:
        return {"source": "keyring", "masked": _mask(kr_val), "env_shadows_keyring": False}
    return {"source": "none", "masked": "", "env_shadows_keyring": False}


def set_api_key(keyring_key: str, value: str) -> bool:
    """Store key in keyring. Returns True on success."""
    try:
        import keyring
        keyring.set_password(_KEYRING_SERVICE, keyring_key, value)
        return True
    except Exception:
        return False


def delete_api_key(keyring_key: str) -> bool:
    try:
        import keyring
        keyring.delete_password(_KEYRING_SERVICE, keyring_key)
        return True
    except Exception:
        return False
