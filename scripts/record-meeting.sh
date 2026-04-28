#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Аргумент — только имя (без пути и расширения)
RAW_NAME="${1:-}"
if [[ -n "$RAW_NAME" && ! "$RAW_NAME" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2} ]]; then
    RAW_NAME="$(date +%Y-%m-%d_%H-%M)_${RAW_NAME}"
fi
RAW_NAME="${RAW_NAME:-$(date +%Y-%m-%d_%H-%M)}"
NAME="$(basename "${RAW_NAME%.wav}")"

# Резолвим meetings_dir: ENV → config.json (XDG) → xdg-user-dir → дефолт
_PY="$ROOT_DIR/.venv/bin/python3"
if [[ -n "${MEETING_ASSISTANT_DATA_DIR:-}" ]]; then
    MEETINGS_DIR="$MEETING_ASSISTANT_DATA_DIR"
elif [[ -x "$_PY" ]]; then
    MEETINGS_DIR="$("$_PY" -c "
import json, os
from pathlib import Path
env = os.environ.get('MEETING_ASSISTANT_CONFIG')
xdg = os.environ.get('XDG_CONFIG_HOME') or str(Path.home() / '.config')
cfg_path = Path(env).expanduser() if env else Path(xdg) / 'meeting-assistant' / 'config.json'
d = ''
if cfg_path.exists():
    d = json.load(open(cfg_path)).get('storage', {}).get('meetings_dir', '')
if not d:
    import subprocess
    try:
        docs = subprocess.check_output(['xdg-user-dir', 'DOCUMENTS'], text=True, timeout=2).strip()
        d = docs + '/meetings' if docs else ''
    except Exception:
        pass
if not d:
    d = str(Path.home() / 'Documents' / 'meetings')
print(os.path.expanduser(d), end='')
")"
else
    MEETINGS_DIR="$(xdg-user-dir DOCUMENTS)/meetings"
fi

MEETING_DIR="$MEETINGS_DIR/$NAME"
OUTPUT="$MEETING_DIR/recording.wav"

# Живой WAV пишем во временный кэш
CACHE_DIR="$HOME/.cache/meeting-assistant/recording"
TS="$(date +%s)"
LIVE_WAV="$CACHE_DIR/current-${TS}.wav"
CLEANED=0

mkdir -p "$MEETING_DIR" "$CACHE_DIR"

# Чистим старые live-файлы (старше 24 ч)
find "$CACHE_DIR" -name "current-*.wav" -mmin +1440 -delete 2>/dev/null || true

show_result() {
    if [[ -f "$OUTPUT" && -s "$OUTPUT" ]]; then
        echo "Файл записан: $OUTPUT"
        # Проверяем, не беззвучна ли запись
        MAX_VOL=$(ffmpeg -i "$OUTPUT" -af "volumedetect" -f null /dev/null 2>&1 \
            | grep -oP 'max_volume:\s*\K[-0-9.]+' | head -1)
        if [[ -n "$MAX_VOL" ]] && awk "BEGIN{exit !($MAX_VOL < -40)}"; then
            echo ""
            echo "ВНИМАНИЕ: Запись почти беззвучна (max_volume: ${MAX_VOL} dB)."
            echo "Возможно, микрофон был выключен во время записи."
        fi
        echo ""
        echo "Для создания протокола запусти:"
        echo "  cd $ROOT_DIR && ./scripts/process-meeting.sh \"$OUTPUT\""
    else
        echo "Файл не создан — возможно, запись не успела начаться."
    fi
}

on_int() {
    [[ "$CLEANED" == 1 ]] && return; CLEANED=1
    echo ""; echo "Останавливаю запись..."
    wait "$REC_PID" 2>/dev/null || true
    if [[ -f "$LIVE_WAV" && -s "$LIVE_WAV" ]]; then
        mv "$LIVE_WAV" "$OUTPUT"
    fi
    show_result
    exit 0
}

on_exit() {
    [[ "$CLEANED" == 1 ]] && return; CLEANED=1
    kill -INT "$REC_PID" 2>/dev/null || true
    wait "$REC_PID" 2>/dev/null || true
    if [[ -f "$LIVE_WAV" && -s "$LIVE_WAV" ]]; then
        mv "$LIVE_WAV" "$OUTPUT"
    fi
    show_result
}

trap on_int INT
trap on_exit EXIT TERM

# Определяем источники: сначала из config.json, затем pactl-дефолты
MIC_SOURCE=""
MONITOR_SOURCE=""
if [[ -f "$_CFG" && -x "$_PY" ]]; then
    MIC_SOURCE="$("$_PY" -c "import json; c=json.load(open('$_CFG')); print(c.get('recording',{}).get('mic_source',''), end='')")"
    MONITOR_SOURCE="$("$_PY" -c "import json; c=json.load(open('$_CFG')); print(c.get('recording',{}).get('system_source',''), end='')")"
fi
MIC_SOURCE="${MIC_SOURCE:-$(pactl get-default-source)}"
MONITOR_SOURCE="${MONITOR_SOURCE:-$(pactl get-default-sink).monitor}"

echo "Микрофон:        $MIC_SOURCE"
echo "Системный аудио: $MONITOR_SOURCE"
echo ""

echo "Папка встречи: $MEETING_DIR"
echo "Запись → $OUTPUT"
echo "Нажми Ctrl+C чтобы остановить."
echo ""

ffmpeg -hide_banner -loglevel error \
    -f pulse -i "$MONITOR_SOURCE" \
    -f pulse -i "$MIC_SOURCE" \
    -filter_complex "[0:a][1:a]amix=inputs=2:duration=longest:normalize=0" \
    -acodec pcm_s16le -ar 16000 -ac 1 \
    "$LIVE_WAV" &
REC_PID=$!
wait "$REC_PID" || true
