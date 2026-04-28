#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PY="$SCRIPT_DIR/.venv/bin/python3"

if [[ ! -x "$VENV_PY" ]]; then
    echo "Ошибка: .venv не найдено. Запусти ./scripts/setup.sh"
    exit 1
fi

PORT=$(python3 -c "import socket; s=socket.socket(); s.bind(('',0)); print(s.getsockname()[1]); s.close()")
export SERVER_PORT="$PORT"

cd "$SCRIPT_DIR"
exec "$VENV_PY" -m uvicorn meeting_assistant.interfaces.web.server:app \
    --host 127.0.0.1 \
    --port "$PORT" \
    --no-access-log
