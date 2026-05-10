#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Проверка зависимостей ==="

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo "  ✗ $1 не найден — $2"
        return 1
    fi
    echo "  ✓ $1"
}

check_cmd python3 "установи python3" || MISSING=1
check_cmd pip3 "установи python3-pip" || MISSING=1
check_cmd pactl "установи pulseaudio-utils (sudo apt install pulseaudio-utils)" || MISSING=1
check_cmd parecord "установи pulseaudio-utils (sudo apt install pulseaudio-utils)" || MISSING=1

if [[ -n "$MISSING" ]]; then
    echo ""
    echo "Установи недостающие пакеты и запусти setup.sh снова."
    exit 1
fi

echo ""
echo "=== Создание виртуального окружения ==="
if [[ ! -d "$ROOT_DIR/.venv" ]]; then
    python3 -m venv "$ROOT_DIR/.venv"
    echo "  ✓ .venv создан"
else
    echo "  ✓ .venv уже существует"
fi

echo ""
echo "=== Установка Python-зависимостей ==="
"$ROOT_DIR/.venv/bin/python3" -m pip install --quiet --upgrade faster-whisper anthropic google-genai mistralai openai fastapi "uvicorn[standard]" pydantic keyring python-multipart rq redis
echo "  ✓ faster-whisper, anthropic, google-genai, mistralai, openai, fastapi, uvicorn, pydantic, keyring, python-multipart, rq, redis"

echo ""
echo "=== Создание директорий ==="
mkdir -p "$ROOT_DIR/meetings" "$ROOT_DIR/recordings"
echo "  ✓ meetings/, recordings/"

echo ""
echo "=== Проверка Ollama (локальные модели) ==="
if ! command -v ollama &>/dev/null; then
    echo "  ○ ollama не установлен (нужен только для локальных моделей)"
    echo "    Установить: curl -fsSL https://ollama.com/install.sh | sh"
else
    echo "  ✓ ollama найден"
    if curl -s --max-time 2 http://localhost:11434/api/tags > /dev/null 2>&1; then
        MODEL_COUNT=$(curl -s http://localhost:11434/api/tags | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('models',[])))" 2>/dev/null || echo "?")
        echo "  ✓ Ollama запущена, моделей установлено: $MODEL_COUNT"
    else
        echo "  ○ Ollama не запущена. Запустить: ollama serve  (или systemctl --user start ollama)"
    fi
    echo "  Рекомендуемая стартовая модель (~9 ГБ, хороший русский):"
    echo "    ollama pull qwen2.5:14b-instruct-q4_K_M"
fi

echo ""
echo "=== Проверка ANTHROPIC_API_KEY ==="
if [[ -z "$ANTHROPIC_API_KEY" ]]; then
    echo "  ✗ Переменная ANTHROPIC_API_KEY не задана."
    echo ""
    echo "  Добавь в ~/.bashrc или ~/.zshrc:"
    echo "    export ANTHROPIC_API_KEY='sk-ant-...'"
    echo ""
    echo "  Ключ получить: https://console.anthropic.com/settings/keys"
else
    echo "  ✓ ANTHROPIC_API_KEY найден"
fi

echo ""
echo "=== Готово ==="
echo "Использование:"
echo "  ./scripts/record-meeting.sh <имя>     # Запустить запись"
echo "  ./scripts/process-meeting.sh <файл>   # Транскрипция + протокол"
