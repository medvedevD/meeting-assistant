#!/usr/bin/env bash
# Fails when the English translation is out of step with the sources.
#
# Runs lupdate on a copy of the .ts and inspects the result, so it catches both a
# committed gap and a string changed in the code without touching the .ts:
#   - new or edited source strings come out as type="unfinished"
#   - removed strings come out as type="vanished" or type="obsolete"
#   - a translation containing Cyrillic is a Russian leftover
#
# Fix: cmake --build <build-dir> --target update_translations, then fill in the
# strings it marks in qt-app/i18n/meeting-assistant_en.ts.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

ts="qt-app/i18n/meeting-assistant_en.ts"

lupdate_bin="${LUPDATE:-}"
if [[ -z "$lupdate_bin" ]]; then
    if command -v lupdate >/dev/null 2>&1; then
        lupdate_bin="lupdate"
    elif [[ -n "${QT_ROOT_DIR:-}" && -x "$QT_ROOT_DIR/bin/lupdate" ]]; then
        lupdate_bin="$QT_ROOT_DIR/bin/lupdate"
    else
        echo "error: lupdate not found (set LUPDATE or QT_ROOT_DIR)" >&2
        exit 2
    fi
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cp "$ts" "$tmp/check.ts"

# Same scope and options as qt_add_translations in qt-app/CMakeLists.txt.
"$lupdate_bin" qt-app/qml qt-app/src -locations none -ts "$tmp/check.ts" >"$tmp/lupdate.log"

sources="$(grep -c '<source>' "$tmp/check.ts" || true)"
if [[ "$sources" -eq 0 ]]; then
    echo "error: lupdate found no strings" >&2
    cat "$tmp/lupdate.log" >&2
    exit 1
fi

# The source text of every message whose translation has the given type.
list_marked() {
    awk -v mark="type=\"$1\"" '
        /<message/       { source = ""; in_source = 0 }
        /<source>/       { in_source = 1 }
        in_source        { source = source $0 "\n" }
        /<\/source>/     { in_source = 0 }
        /<translation/ && index($0, mark) { printf "%s", source }
    ' "$tmp/check.ts"
}

status=0
for mark in unfinished vanished obsolete; do
    found="$(list_marked "$mark")"
    if [[ -n "$found" ]]; then
        echo "error: $mark strings in $ts:" >&2
        echo "$found" >&2
        status=1
    fi
done

# Byte ranges of UTF-8 Cyrillic (U+0400..U+04FF), so this needs no UTF-8 locale.
# grep has no multi-line mode, so awk first keeps only the translation text.
cyrillic="$(
    awk '
        /<translation/   { in_translation = 1 }
        in_translation   { print }
        /<\/translation>/ { in_translation = 0 }
    ' "$tmp/check.ts" | LC_ALL=C grep -E $'[\xD0-\xD3][\x80-\xBF]' || true
)"
if [[ -n "$cyrillic" ]]; then
    echo "error: Cyrillic in English translations:" >&2
    echo "$cyrillic" >&2
    status=1
fi

if [[ "$status" -ne 0 ]]; then
    echo "Run 'cmake --build <build-dir> --target update_translations' and translate." >&2
    exit "$status"
fi

echo "ok: $sources strings translated in $ts"
