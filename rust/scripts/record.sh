#!/usr/bin/env bash
# Record meeting audio via the meeting-assistant server.
#
# Usage:
#   ./scripts/record.sh start [--source mic|system|mixed] [--name "Meeting name"]
#   ./scripts/record.sh stop
#   ./scripts/record.sh status
#   ./scripts/record.sh transcribe [<wav-path>]
#   ./scripts/record.sh generate [--name "Meeting name"] [<transcript-path>]
#
# The active session ID is stored in /tmp/meeting-assistant-session so start/stop
# work across two separate terminal invocations without copy-pasting anything.
# The last WAV and transcript paths are stored similarly for transcribe/generate.
#
# Env:
#   MEETING_PORT   Server port (default 18080)

set -euo pipefail

PORT="${MEETING_PORT:-18080}"
BASE="http://127.0.0.1:$PORT/api/v1"
SESSION_FILE="/tmp/meeting-assistant-session"
LAST_MEETING_FILE="/tmp/meeting-assistant-last-meeting"
LAST_WAV_FILE="/tmp/meeting-assistant-last-wav"
LAST_TRANSCRIPT_FILE="/tmp/meeting-assistant-last-transcript"

_json_field() {
    # Extract a field from a JSON string without jq dependency
    python3 -c "import sys, json; print(json.loads(sys.argv[1])['$2'])" "$1"
}

_check_server() {
    if ! curl -s --max-time 1 "$BASE/meetings" > /dev/null 2>&1; then
        echo "Server is not running on port $PORT."
        echo "Start it with: ./scripts/serve.sh"
        exit 1
    fi
}

cmd="${1:-}"
shift || true

case "$cmd" in
    start)
        SOURCE="mic"
        NAME=""
        ECHO_CANCEL="false"
        while [[ $# -gt 0 ]]; do
            case $1 in
                --source)       SOURCE="$2"; shift 2 ;;
                --name)         NAME="$2";   shift 2 ;;
                --echo-cancel)  ECHO_CANCEL="true"; shift ;;
                *) echo "Unknown argument: $1"; exit 1 ;;
            esac
        done

        _check_server

        BODY=$(python3 -c "
import json, sys
d = {'source': sys.argv[1], 'echo_cancel': sys.argv[3] == 'true'}
if sys.argv[2]: d['name'] = sys.argv[2]
print(json.dumps(d))
" "$SOURCE" "$NAME" "$ECHO_CANCEL")

        RESP=$(curl -s -w '\n%{http_code}' -X POST "$BASE/recordings" \
            -H "Content-Type: application/json" \
            -d "$BODY")

        HTTP_CODE=$(echo "$RESP" | tail -1)
        JSON=$(echo "$RESP" | head -1)

        if [ "$HTTP_CODE" != "201" ]; then
            echo "Error $HTTP_CODE: $JSON"
            exit 1
        fi

        ID=$(_json_field "$JSON" id)
        AUDIO=$(_json_field "$JSON" audio_path)

        echo "$ID" > "$SESSION_FILE"
        echo "$ID" > "$LAST_MEETING_FILE"
        echo "Recording started"
        echo "  id:     $ID"
        echo "  source: $SOURCE"
        echo "  file:   $AUDIO"
        echo ""
        echo "Stop with: ./scripts/record.sh stop"
        ;;

    stop)
        if [ ! -f "$SESSION_FILE" ]; then
            echo "No active session. Start one with: ./scripts/record.sh start"
            exit 1
        fi

        ID=$(cat "$SESSION_FILE")
        _check_server

        RESP=$(curl -s -w '\n%{http_code}' -X POST "$BASE/recordings/$ID/stop")
        HTTP_CODE=$(echo "$RESP" | tail -1)
        JSON=$(echo "$RESP" | head -1)

        if [ "$HTTP_CODE" != "200" ]; then
            echo "Error $HTTP_CODE: $JSON"
            exit 1
        fi

        AUDIO=$(_json_field "$JSON" audio_path)
        rm -f "$SESSION_FILE"
        echo "$AUDIO" > "$LAST_WAV_FILE"

        echo "Recording stopped"
        echo "  file: $AUDIO"
        echo "  size: $(du -h "$AUDIO" 2>/dev/null | cut -f1 || echo 'n/a')"
        echo ""
        echo "Next: ./scripts/record.sh transcribe"
        ;;

    transcribe)
        WAV="${1:-}"
        if [ -z "$WAV" ]; then
            if [ ! -f "$LAST_WAV_FILE" ]; then
                echo "No WAV path. Pass it as argument or run 'stop' first."
                exit 1
            fi
            WAV=$(cat "$LAST_WAV_FILE")
        fi

        MEETING_ID=""
        if [ -f "$LAST_MEETING_FILE" ]; then
            MEETING_ID=$(cat "$LAST_MEETING_FILE")
        fi

        _check_server

        BODY=$(python3 -c "
import json, sys
d = {'path': sys.argv[1]}
if sys.argv[2]: d['meeting_id'] = sys.argv[2]
print(json.dumps(d))
" "$WAV" "$MEETING_ID")

        RESP=$(curl -s -w '\n%{http_code}' -X POST "$BASE/transcribe" \
            -H "Content-Type: application/json" \
            -d "$BODY")

        HTTP_CODE=$(echo "$RESP" | tail -1)
        JSON=$(echo "$RESP" | head -1)

        if [ "$HTTP_CODE" != "200" ]; then
            echo "Error $HTTP_CODE: $JSON"
            exit 1
        fi

        TEXT=$(_json_field "$JSON" text)
        TRANSCRIPT="$(dirname "$WAV")/transcript.txt"
        echo "$TEXT" > "$TRANSCRIPT"
        echo "$TRANSCRIPT" > "$LAST_TRANSCRIPT_FILE"

        echo "Transcript saved: $TRANSCRIPT"
        echo "  words: $(wc -w < "$TRANSCRIPT")"
        echo ""
        echo "Next: ./scripts/record.sh generate --name \"<имя встречи>\""
        ;;

    generate)
        NAME=""
        TRANSCRIPT=""
        while [[ $# -gt 0 ]]; do
            case $1 in
                --name) NAME="$2"; shift 2 ;;
                *)      TRANSCRIPT="$1"; shift ;;
            esac
        done

        if [ -z "$TRANSCRIPT" ]; then
            if [ ! -f "$LAST_TRANSCRIPT_FILE" ]; then
                echo "No transcript path. Pass it as argument or run 'transcribe' first."
                exit 1
            fi
            TRANSCRIPT=$(cat "$LAST_TRANSCRIPT_FILE")
        fi

        if [ ! -f "$TRANSCRIPT" ]; then
            echo "Transcript file not found: $TRANSCRIPT"
            exit 1
        fi

        _check_server

        BODY=$(python3 -c "
import json, sys
d = {'transcript': open(sys.argv[1]).read()}
if sys.argv[2]: d['meeting_name'] = sys.argv[2]
print(json.dumps(d))
" "$TRANSCRIPT" "$NAME")

        RESP=$(curl -s -w '\n%{http_code}' -X POST "$BASE/protocols" \
            -H "Content-Type: application/json" \
            -d "$BODY")

        HTTP_CODE=$(echo "$RESP" | tail -1)
        JSON=$(echo "$RESP" | head -1)

        if [ "$HTTP_CODE" != "200" ]; then
            echo "Error $HTTP_CODE: $JSON"
            exit 1
        fi

        MARKDOWN=$(_json_field "$JSON" markdown)
        PROTOCOL="$(dirname "$TRANSCRIPT")/protocol.md"
        printf '%s\n' "$MARKDOWN" > "$PROTOCOL"

        echo "Protocol saved: $PROTOCOL"
        ;;

    status)
        if [ ! -f "$SESSION_FILE" ]; then
            echo "No active session."
        else
            ID=$(cat "$SESSION_FILE")
            echo "Active session: $ID"
        fi

        _check_server
        echo ""
        echo "Meetings:"
        curl -s "$BASE/meetings" | python3 -c "
import sys, json
meetings = json.load(sys.stdin)
if not meetings:
    print('  (none)')
for m in meetings:
    mark = '✓' if m.get('has_transcript') else ' '
    print(f'  [{mark}] {m[\"name\"]}  {m[\"id\"][:8]}...')
"
        ;;

    *)
        echo "Usage: $0 start [--source mic|system|mixed] [--name <name>] [--echo-cancel]"
        echo "       $0 stop"
        echo "       $0 transcribe [<wav-path>]"
        echo "       $0 generate [--name <name>] [<transcript-path>]"
        echo "       $0 status"
        exit 1
        ;;
esac
