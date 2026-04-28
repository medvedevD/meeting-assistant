"""CLI: транскрипция + протокол встречи."""

import sys
import os
import argparse
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
ROOT_DIR = SCRIPT_DIR.parent

sys.path.insert(0, str(SCRIPT_DIR))
sys.path.insert(0, str(ROOT_DIR))

WHISPER_MODELS = ["tiny", "small", "medium", "large-v3"]


def main():
    from meeting_assistant.infrastructure.container import build_container
    from meeting_assistant.application.process_meeting import ProcessMeetingRequest

    container = build_container()
    default_model = container.config.get("transcription", "model", "medium")

    parser = argparse.ArgumentParser(description="Транскрипция + протокол встречи")
    parser.add_argument("audio", help="Путь к аудиофайлу (или папке при --from-transcript)")
    parser.add_argument("name", nargs="?", help="Имя встречи (по умолчанию — имя файла)")
    parser.add_argument(
        "--model", "-m",
        choices=WHISPER_MODELS,
        default=default_model,
        help=f"Модель Whisper (по умолчанию из config: {default_model})",
    )
    parser.add_argument(
        "--no-protocol",
        action="store_true",
        help="Только транскрипция, без генерации протокола",
    )
    parser.add_argument(
        "--from-transcript",
        action="store_true",
        help="Читать транскрипцию из существующего transcript.md, пропустить Whisper",
    )
    args = parser.parse_args()

    if not args.from_transcript and not os.path.exists(args.audio):
        sys.exit(f"Файл не найден: {args.audio}")

    audio_path = Path(args.audio).resolve()
    meeting_name = args.name or audio_path.stem

    request = ProcessMeetingRequest(
        audio_path=audio_path,
        meeting_name=meeting_name,
        whisper_model=args.model,
        generate_protocol=not args.no_protocol,
        from_transcript=args.from_transcript,
    )

    def on_stage(stage: str) -> None:
        print(f"__STAGE:{stage}__", flush=True)

    container.process_meeting.execute(request, on_stage=on_stage)


if __name__ == "__main__":
    main()
