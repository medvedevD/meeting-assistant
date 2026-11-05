#!/usr/bin/env bash
# Fails when MeetingAssistant.app breaks what it promises its users:
#   1. Every Mach-O runs on the macOS named by LSMinimumSystemVersion.
#   2. The programs in Contents/MacOS are arm64 only.
#   3. Qt-VERSION.txt, which the LGPL notice points at, names the bundled Qt.
#
# (1) needs a check because nothing else notices: a compile with no deployment
# target builds for the OS it runs on, so a newer runner raises the real minimum
# without an error anywhere. v0.1.0-alpha.13, built on macos-14, shipped a GUI and
# an ffmpeg marked minos 14.0 behind a plist that says 13.0.
#
# Usage: check-bundle.sh /path/to/MeetingAssistant.app
set -euo pipefail

APP="${1:?usage: check-bundle.sh /path/to/MeetingAssistant.app}"
[[ -d "$APP" ]] || { echo "Error: not a bundle: $APP" >&2; exit 1; }

floor="$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$APP/Contents/Info.plist")"
status=0
checked=0

# 1. Minimum OS. A file with no arm64 slice reads as unknown here and fails too.
minos_re='minos ([0-9.]+)'
while IFS= read -r -d '' f; do
    [[ "$(file -b "$f")" == *Mach-O* ]] || continue
    checked=$((checked + 1))
    minos=""
    build="$(vtool -show-build -arch arm64 "$f" 2>/dev/null || true)"
    [[ "$build" =~ $minos_re ]] && minos="${BASH_REMATCH[1]}"
    if [[ -z "$minos" || "$(printf '%s\n%s\n' "$minos" "$floor" | sort -V | tail -n 1)" != "$floor" ]]; then
        echo "Error: ${f#"$APP"/} needs macOS ${minos:-unknown}, but the app promises $floor" >&2
        status=1
    fi
done < <(find "$APP/Contents" -type f -print0)

if [[ "$checked" -eq 0 ]]; then
    echo "Error: no Mach-O files under $APP/Contents" >&2
    status=1
fi

# 2. Architecture of the programs themselves. The Qt frameworks stay universal,
# as Qt ships them.
for f in "$APP/Contents/MacOS/"*; do
    archs="$(lipo -archs "$f" 2>/dev/null || true)"
    if [[ "$archs" != "arm64" ]]; then
        echo "Error: ${f#"$APP"/} is '${archs:-not Mach-O}', expected arm64" >&2
        status=1
    fi
done

# 3. The version the notice names against the QtCore in the bundle. The first
# "current version" otool prints is the library's own.
qtcore="$APP/Contents/Frameworks/QtCore.framework/Versions/A/QtCore"
notice="$APP/Contents/Resources/licenses/Qt-VERSION.txt"
version_re='current version ([0-9.]+)'
bundled=""
linkage="$(otool -L "$qtcore" 2>/dev/null || true)"
[[ "$linkage" =~ $version_re ]] && bundled="${BASH_REMATCH[1]}"
named="$(head -n 1 "$notice" 2>/dev/null || true)"
if [[ -z "$bundled" || "$named" != "Qt $bundled" ]]; then
    echo "Error: Qt-VERSION.txt says '${named:-nothing}', but the bundle has Qt ${bundled:-unknown}" >&2
    status=1
fi

[[ "$status" -eq 0 ]] || exit "$status"
echo "✓ $checked Mach-O files run on macOS $floor; programs are arm64; notice names Qt $bundled"
