#!/usr/bin/env bash
# Benchmark whisper transcription across all available GGML models.
#
# Usage: ./scripts/bench_transcribe.sh [--build] [FILE]
#   --build   Rebuild release binary before running
#   FILE      Audio file to transcribe (WAV)
#
# Env:
#   MODELS_DIR   Directory with ggml-*.bin files
#                (default: ~/.local/share/meeting-assistant/models)

set -euo pipefail
export LC_NUMERIC=C

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$SCRIPT_DIR/.."
BIN="$RUST_DIR/target/release/meeting-assistant"
MODELS_DIR="${MODELS_DIR:-$HOME/.local/share/meeting-assistant/models}"
DEFAULT_FILE="/home/dmitry/Документы/meeting-assistant/2026-05-10_16-02_1910df7a/recording.wav"

BUILD=false
AUDIO_FILE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --build) BUILD=true; shift ;;
        -*) echo "Unknown option: $1"; exit 1 ;;
        *)  AUDIO_FILE="$1"; shift ;;
    esac
done

AUDIO_FILE="${AUDIO_FILE:-$DEFAULT_FILE}"

# --- Build ---
if $BUILD; then
    echo "Building release binary..."
    (cd "$RUST_DIR" && cargo build --release --bin meeting-assistant)
    echo ""
fi

if [ ! -x "$BIN" ]; then
    echo "Binary not found: $BIN"
    echo "Run with --build or: cd rust && cargo build --release --bin meeting-assistant"
    exit 1
fi

if [ ! -f "$AUDIO_FILE" ]; then
    echo "Audio file not found: $AUDIO_FILE"
    exit 1
fi

# --- Audio duration ---
DURATION=""
if command -v ffprobe &>/dev/null; then
    DURATION=$(LC_ALL=C ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$AUDIO_FILE" 2>/dev/null || true)
else
    echo "Warning: ffprobe not found — RTF will not be calculated"
fi

FILE_SIZE=$(du -sh "$AUDIO_FILE" | cut -f1)
FILE_NAME=$(basename "$AUDIO_FILE")

echo "=== Transcription Benchmark ==="
echo "Audio file : $FILE_NAME ($FILE_SIZE)"
if [ -n "$DURATION" ]; then
    printf "Duration   : %.1f s\n" "$DURATION"
fi
echo "Binary     : $BIN"
echo ""

# --- Discover models ---
mapfile -t MODELS < <(find "$MODELS_DIR" -maxdepth 1 -name "*.bin" 2>/dev/null | sort)

if [ ${#MODELS[@]} -eq 0 ]; then
    echo "No models found in: $MODELS_DIR"
    echo "Expected files like ggml-base.bin, ggml-medium.bin"
    exit 1
fi

echo "Running ${#MODELS[@]} model(s)..."
echo ""

declare -a RESULT_NAMES RESULT_SIZES RESULT_TIMES RESULT_RTFS

for i in "${!MODELS[@]}"; do
    MODEL="${MODELS[$i]}"
    MODEL_NAME=$(basename "$MODEL")
    MODEL_SIZE=$(du -sh "$MODEL" | cut -f1)
    IDX=$((i + 1))

    printf "[%d/%d] %s (%s)\n" "$IDX" "${#MODELS[@]}" "$MODEL_NAME" "$MODEL_SIZE"

    START_NS=$(date +%s%N)
    MEETING_ASSISTANT_MODEL="$MODEL" "$BIN" transcribe "$AUDIO_FILE" > /dev/null 2>&1
    END_NS=$(date +%s%N)

    ELAPSED_MS=$(( (END_NS - START_NS) / 1000000 ))
    ELAPSED_S=$(echo "scale=1; $ELAPSED_MS / 1000" | bc)

    RTF_STR="n/a"
    if [ -n "$DURATION" ] && command -v bc &>/dev/null; then
        RTF=$(echo "scale=2; $ELAPSED_MS / 1000 / $DURATION" | bc)
        RTF_STR="$RTF"
    fi

    printf "      Time: %s s   RTF: %s\n\n" "$ELAPSED_S" "$RTF_STR"

    RESULT_NAMES+=("$MODEL_NAME")
    RESULT_SIZES+=("$MODEL_SIZE")
    RESULT_TIMES+=("${ELAPSED_S}s")
    RESULT_RTFS+=("$RTF_STR")
done

# --- Summary table ---
echo "=== Summary ==="
printf "%-24s %-8s %-10s %s\n" "Model" "Size" "Time" "RTF"
printf "%-24s %-8s %-10s %s\n" "-----" "----" "----" "---"
for i in "${!RESULT_NAMES[@]}"; do
    printf "%-24s %-8s %-10s %s\n" \
        "${RESULT_NAMES[$i]}" \
        "${RESULT_SIZES[$i]}" \
        "${RESULT_TIMES[$i]}" \
        "${RESULT_RTFS[$i]}"
done
