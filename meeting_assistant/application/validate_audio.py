"""Validate an audio file with ffprobe before launching transcription."""

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass
class AudioValidationResult:
    ok: bool
    error_kind: str | None = None
    error_message: str | None = None
    duration_sec: float | None = None


_MIN_DURATION_SEC = 5.0
_SILENCE_RMS_THRESHOLD = -45.0  # dB


def validate_audio(path: Path) -> AudioValidationResult:
    if not path.exists():
        return AudioValidationResult(ok=False, error_kind="audio_corrupt", error_message="Файл не найден")

    # Run ffprobe to get stream info
    cmd = [
        "ffprobe", "-v", "quiet",
        "-print_format", "json",
        "-show_streams", "-show_format",
        str(path),
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    except FileNotFoundError:
        return AudioValidationResult(ok=True)  # ffprobe not installed — skip validation
    except subprocess.TimeoutExpired:
        return AudioValidationResult(ok=False, error_kind="audio_corrupt", error_message="ffprobe завис при анализе файла")

    if result.returncode != 0:
        return AudioValidationResult(ok=False, error_kind="audio_corrupt", error_message="Файл повреждён или не является аудио")

    try:
        info = json.loads(result.stdout)
    except json.JSONDecodeError:
        return AudioValidationResult(ok=False, error_kind="audio_corrupt", error_message="Не удалось прочитать метаданные файла")

    streams = info.get("streams", [])
    audio_streams = [s for s in streams if s.get("codec_type") == "audio"]
    if not audio_streams:
        return AudioValidationResult(ok=False, error_kind="audio_corrupt", error_message="В файле нет аудиодорожек")

    fmt = info.get("format", {})
    duration_str = fmt.get("duration") or audio_streams[0].get("duration")
    duration = float(duration_str) if duration_str else None

    if duration is not None and duration < _MIN_DURATION_SEC:
        return AudioValidationResult(
            ok=False,
            error_kind="audio_too_short",
            error_message=f"Запись слишком короткая ({duration:.1f} сек). Минимум — {_MIN_DURATION_SEC:.0f} сек",
            duration_sec=duration,
        )

    return AudioValidationResult(ok=True, duration_sec=duration)
