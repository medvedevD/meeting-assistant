from collections.abc import Callable
from pathlib import Path

from ...domain.ports.transcriber import ITranscriber
from ...domain.ports.config_provider import IConfigProvider
from ...domain.entities.recording import Recording
from ...domain.entities.transcript import Transcript
from ...domain.value_objects.transcript_segment import TranscriptSegment


def _is_model_cached(model: str) -> bool:
    cache_dir = Path.home() / ".cache" / "huggingface" / "hub"
    return any(cache_dir.glob(f"models--Systran--faster-whisper-{model}*"))


class FasterWhisperTranscriber(ITranscriber):
    def __init__(self, config: IConfigProvider):
        self._config = config

    def transcribe(
        self,
        recording: Recording,
        model_name: str,
        on_progress: Callable[[float], None] | None = None,
    ) -> Transcript:
        from faster_whisper import WhisperModel

        device = self._config.get("transcription", "device", "cpu")
        compute_type = self._config.get("transcription", "compute_type", "int8")
        beam_size = self._config.get("transcription", "beam_size", 5)

        if _is_model_cached(model_name):
            print(f"Загружаю модель Whisper ({model_name}, {device})...", flush=True)
        else:
            sizes = {"tiny": "75 МБ", "small": "244 МБ", "medium": "1.5 ГБ", "large-v3": "3 ГБ"}
            size = sizes.get(model_name, "?")
            print(f"Загружаю модель Whisper ({model_name}, первый раз — скачает ~{size})...", flush=True)
        model = WhisperModel(model_name, device=device, compute_type=compute_type)

        language = self._config.get("transcription", "language", "ru")
        vad_filter = self._config.get("transcription", "vad_filter", True)
        min_silence = self._config.get("transcription", "min_silence_duration_ms", 500)
        speech_pad = self._config.get("transcription", "speech_pad_ms", 200)

        raw_segments, info = model.transcribe(
            str(recording.path),
            language=language,
            beam_size=beam_size,
            vad_filter=vad_filter,
            vad_parameters=dict(min_silence_duration_ms=min_silence, speech_pad_ms=speech_pad),
        )

        total_duration = info.duration or 0.0
        segments = []
        for seg in raw_segments:
            segments.append(TranscriptSegment(start=seg.start, end=seg.end, text=seg.text.strip()))
            if on_progress and total_duration > 0:
                on_progress(min(seg.end / total_duration, 1.0))

        if on_progress:
            on_progress(1.0)

        print(f"  ✓ Готово — язык: {info.language} ({info.language_probability:.0%})", flush=True)

        return Transcript(
            segments=segments,
            language=info.language,
            language_probability=info.language_probability,
        )
