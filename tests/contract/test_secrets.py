"""
Contract tests for secrets.get_api_key_info().
Verifies source detection and masking without touching real env or keyring.
"""
import os
from unittest.mock import patch, MagicMock
import pytest

from meeting_assistant.adapters.config.secrets import get_api_key_info, _mask


# ── _mask ──────────────────────────────────────────────────────────────────────

class TestMask:
    def test_normal_key(self):
        result = _mask("sk-ant-abcdefghijklmnop")
        assert result.startswith("sk-ant-")
        assert "***" in result
        assert result.endswith("mnop")

    def test_short_key_returns_stars(self):
        assert _mask("short") == "***"

    def test_exactly_12_chars(self):
        result = _mask("a" * 12)
        assert "***" in result


# ── get_api_key_info ───────────────────────────────────────────────────────────

class TestGetApiKeyInfo:
    def test_env_source(self):
        with patch.dict(os.environ, {"MY_KEY": "sk-ant-abcdefghijklmnopqrst"}):
            info = get_api_key_info("MY_KEY", "my_key")
        assert info["source"] == "env"
        assert "***" in info["masked"]
        assert info["env_shadows_keyring"] is False

    def test_keyring_source(self):
        with patch.dict(os.environ, {}, clear=False):
            # ensure env var absent
            os.environ.pop("MY_KEY2", None)
            with patch("meeting_assistant.adapters.config.secrets._keyring_get",
                       return_value="kr-secret-abcdefghijklmnop"):
                info = get_api_key_info("MY_KEY2", "my_key2")
        assert info["source"] == "keyring"
        assert "***" in info["masked"]
        assert info["env_shadows_keyring"] is False

    def test_none_source(self):
        os.environ.pop("MY_KEY3", None)
        with patch("meeting_assistant.adapters.config.secrets._keyring_get", return_value=""):
            info = get_api_key_info("MY_KEY3", "my_key3")
        assert info["source"] == "none"
        assert info["masked"] == ""
        assert info["env_shadows_keyring"] is False

    def test_env_shadows_keyring(self):
        with patch.dict(os.environ, {"MY_KEY4": "sk-ant-abcdefghijklmnopqrst"}):
            with patch("meeting_assistant.adapters.config.secrets._keyring_get",
                       return_value="kr-also-set-abcdefghijklmno"):
                info = get_api_key_info("MY_KEY4", "my_key4")
        assert info["source"] == "env"
        assert info["env_shadows_keyring"] is True

    def test_env_priority_over_keyring(self):
        with patch.dict(os.environ, {"MY_KEY5": "env-value-abcdefghijklmno"}):
            with patch("meeting_assistant.adapters.config.secrets._keyring_get",
                       return_value="kr-value-abcdefghijklmnop"):
                info = get_api_key_info("MY_KEY5", "my_key5")
        assert info["source"] == "env"
        assert info["masked"].startswith("env-val")
