"""Structured logging configuration for meeting-assistant."""
import logging
import logging.handlers
from pathlib import Path

_LOG_DIR = Path.home() / ".local" / "share" / "meeting-assistant" / "logs"
_LOG_FILE = _LOG_DIR / "meeting-assistant.log"

_configured = False


def configure_logging(level: int = logging.INFO) -> None:
    global _configured
    if _configured:
        return
    _configured = True

    root = logging.getLogger("meeting_assistant")
    root.setLevel(logging.DEBUG)

    fmt = logging.Formatter(
        "%(asctime)s  %(levelname)-8s  %(name)s  %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    console = logging.StreamHandler()
    console.setLevel(level)
    console.setFormatter(fmt)
    root.addHandler(console)

    try:
        _LOG_DIR.mkdir(parents=True, exist_ok=True)
        fh = logging.handlers.RotatingFileHandler(
            _LOG_FILE,
            maxBytes=5 * 1024 * 1024,
            backupCount=3,
            encoding="utf-8",
        )
        fh.setLevel(logging.DEBUG)
        fh.setFormatter(fmt)
        root.addHandler(fh)
    except OSError as exc:
        root.warning("Cannot open log file %s: %s", _LOG_FILE, exc)
