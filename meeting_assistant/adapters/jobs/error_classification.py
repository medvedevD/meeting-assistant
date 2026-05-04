"""Map exceptions to error_kind strings as defined in the plan."""

# Errors that warrant automatic retry with backoff.
RETRYABLE_ERRORS: frozenset[str] = frozenset({"api_quota", "network_timeout", "worker_killed"})

# Backoff delays (seconds) indexed by attempt number (0-based).
RETRY_BACKOFF: list[int] = [30, 120, 600]


def classify_error(exc: Exception) -> str:
    msg = str(exc).lower()
    typ = type(exc).__name__.lower()

    if "memoryerror" in typ or "out of memory" in msg or "oom" in msg:
        return "transcription_oom"
    if "cudaoutofmemory" in typ:
        return "transcription_oom"
    if "authentication" in msg or "api key" in msg or "invalid key" in msg or "unauthorized" in msg:
        return "api_auth"
    if "rate limit" in msg or "quota" in msg or "429" in msg:
        return "api_quota"
    if "timeout" in msg or "timed out" in msg:
        return "network_timeout"
    if "corrupt" in msg or "invalid data" in msg or "cannot read" in msg:
        return "audio_corrupt"
    if "too short" in msg or "audio_too_short" in msg:
        return "audio_too_short"
    if "silent" in msg or "audio_silent" in msg:
        return "audio_silent"
    if "template" in msg and ("not found" in msg or "render" in msg):
        return "template_render"
    return "unknown"
