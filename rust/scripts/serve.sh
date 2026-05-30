#!/usr/bin/env bash
# Start the meeting-assistant HTTP server.
#
# Usage: ./scripts/serve.sh [--port 18080] [--bg]
#   --port  Port to listen on (default 18080)
#   --bg    Run in background, write PID to /tmp/meeting-assistant.pid
#
# Env:
#   ANTHROPIC_API_KEY   Required for protocol generation
#   MEETING_PORT        Alternative to --port

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$SCRIPT_DIR/../target/release/meeting-assistant"

if [ ! -x "$BIN" ]; then
    echo "Binary not found: $BIN"
    echo "Run: cd rust && cargo build --release --bin meeting-assistant"
    exit 1
fi

PORT="${MEETING_PORT:-18080}"
BG=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --port) PORT="$2"; shift 2 ;;
        --bg)   BG=true; shift ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
    echo "Warning: ANTHROPIC_API_KEY is not set (protocol generation will fail)"
fi

export MEETING_PORT="$PORT"

if $BG; then
    PID_FILE=/tmp/meeting-assistant.pid
    "$BIN" serve --port "$PORT" &
    echo $! > "$PID_FILE"
    echo "Server started in background (PID $(cat $PID_FILE), port $PORT)"
    echo "Stop with: kill \$(cat $PID_FILE)"
else
    exec "$BIN" serve --port "$PORT"
fi
