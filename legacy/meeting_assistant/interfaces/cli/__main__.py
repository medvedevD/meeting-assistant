"""
python -m meeting_assistant <command>

Commands:
  reindex   Rebuild the SQLite search index from the meetings/ directory.
"""
import sys


def cmd_reindex(args: list[str]) -> None:
    from ...infrastructure.container import build_container
    from ...infrastructure.logging_setup import configure_logging
    configure_logging()
    container = build_container()
    meetings_dir = container.meeting_repo.meetings_dir
    print(f"Индексирую встречи из {meetings_dir} …")
    count = container.search_index.reindex(meetings_dir)
    print(f"✓ Проиндексировано встреч: {count}")


_COMMANDS = {
    "reindex": cmd_reindex,
}


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] not in _COMMANDS:
        print("Использование: python -m meeting_assistant <команда>")
        print("Доступные команды:", ", ".join(_COMMANDS))
        sys.exit(1)
    _COMMANDS[sys.argv[1]](sys.argv[2:])


if __name__ == "__main__":
    main()
