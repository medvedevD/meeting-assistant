#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
VENV_PYTHON="$ROOT_DIR/.venv/bin/python3"

if [[ ! -x "$VENV_PYTHON" ]]; then
    echo "Ошибка: виртуальное окружение не найдено. Запусти ./scripts/setup.sh"
    exit 1
fi

exec "$VENV_PYTHON" "$SCRIPT_DIR/process_meeting.py" "$@"
