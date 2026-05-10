#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_PY="$SCRIPT_DIR/.venv/bin/python3"
VENV_RQ="$SCRIPT_DIR/.venv/bin/rq"

if [[ ! -x "$VENV_PY" ]]; then
    echo "Ошибка: .venv не найдено. Запусти ./scripts/setup.sh"
    exit 1
fi

# ── опциональный RQ-режим ─────────────────────────────────────────────────────
# Включается автоматически если:
#   - docker доступен И образ redis запускается успешно, ИЛИ
#   - redis уже запущен на localhost:6379
# Принудительно отключить: JOB_RUNNER=legacy ./gui.sh
# Принудительно включить:  JOB_RUNNER=rq ./gui.sh

WORKER_PID=""

cleanup() {
    if [[ -n "$WORKER_PID" ]]; then
        kill "$WORKER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

_start_rq() {
    export JOB_RUNNER=rq
    export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"

    # Запускаем воркер в фоне
    "$VENV_RQ" worker transcribe protocol \
        --url "$REDIS_URL" \
        --logging_level WARNING \
        > "$SCRIPT_DIR/.rq-worker.log" 2>&1 &
    WORKER_PID=$!
    echo "  ✓ RQ worker запущен (pid $WORKER_PID, лог: .rq-worker.log)"
}

_start_redis_local() {
    # Запускаем redis-server в фоне (только если ещё не запущен)
    redis-server --daemonize yes \
        --logfile "$SCRIPT_DIR/.redis.log" \
        --port 6379 \
        --save "" \
        2>/dev/null || return 1
    sleep 0.3
    _redis_alive || return 1
    echo "  ✓ Redis запущен локально (лог: .redis.log)"
    return 0
}

_start_redis_docker() {
    # Переиспользуем уже существующий контейнер или запускаем новый
    if docker start meeting-redis 2>/dev/null; then
        echo "  ✓ Redis контейнер переиспользован (meeting-redis)"
        sleep 0.5
        return 0
    fi
    if docker run -d --name meeting-redis -p 6379:6379 \
            redis:7-alpine redis-server --save "" --appendonly no \
            2>/dev/null; then
        echo "  ✓ Redis запущен через docker (контейнер meeting-redis)"
        sleep 1
        return 0
    fi
    return 1
}

_redis_alive() {
    "$VENV_PY" -c "
import socket, sys
try:
    s = socket.create_connection(('127.0.0.1', 6379), timeout=1)
    s.close(); sys.exit(0)
except Exception:
    sys.exit(1)
" 2>/dev/null
}

if [[ "${JOB_RUNNER:-}" == "legacy" ]]; then
    echo "  ○ RQ отключён (JOB_RUNNER=legacy)"
elif [[ "${JOB_RUNNER:-}" == "rq" ]]; then
    echo "  → RQ включён принудительно"
    _start_rq
elif _redis_alive; then
    echo "  → Redis уже запущен, включаю RQ pipeline"
    _start_rq
elif command -v redis-server &>/dev/null && _start_redis_local; then
    _start_rq
elif command -v docker &>/dev/null && _start_redis_docker; then
    _start_rq
else
    echo "  ○ Redis недоступен — работаю в legacy-режиме (без RQ)"
    echo "    Чтобы включить RQ: docker compose up -d redis && ./gui.sh"
fi

# ── запуск веб-сервера ────────────────────────────────────────────────────────
PORT=$(python3 -c "import socket; s=socket.socket(); s.bind(('',0)); print(s.getsockname()[1]); s.close()")
export SERVER_PORT="$PORT"

cd "$SCRIPT_DIR"
exec "$VENV_PY" -m uvicorn meeting_assistant.interfaces.web.server:app \
    --host 127.0.0.1 \
    --port "$PORT" \
    --no-access-log
