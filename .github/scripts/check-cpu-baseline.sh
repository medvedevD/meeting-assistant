#!/usr/bin/env bash
# Fails when a meeting-server binary asks more of the CPU than the release baseline
# set in .cargo/config.toml. See "CPU baseline" in packaging/README.md.
#
# This needs a check because nothing else notices: a binary tuned for a better CPU
# passes every test on the runner that built it. The Windows build of v0.1.0-alpha.13
# shipped AVX-512 that way.
#
#   x86-64  No zmm register anywhere. It reads the code, because a PE keeps no symbols.
#   arm64   ggml built without i8mm, SVE and SME, which the M1 lacks. A native build
#           on any later chip turns on at least one of them.
#
# Usage: check-cpu-baseline.sh <meeting-server binary>
set -euo pipefail

BIN="${1:?usage: check-cpu-baseline.sh <meeting-server binary>}"
[[ -f "$BIN" ]] || { echo "Error: not a file: $BIN" >&2; exit 1; }

# binutils in the Linux container, LLVM elsewhere. Both print AT&T syntax by default.
OBJDUMP="$(command -v objdump || command -v llvm-objdump || true)"
[[ -n "$OBJDUMP" ]] || { echo "Error: no objdump or llvm-objdump on PATH" >&2; exit 1; }

stale_hint() {
    echo "cargo does not rebuild whisper-rs-sys when a GGML_* value changes, so this is" >&2
    echo "usually an old build: run 'cargo clean -p whisper-rs-sys' locally, or change" >&2
    echo "the whisper-baseline part of the rust-cache key in CI." >&2
}

kind="$(file -b "$BIN")"
case "$kind" in
    *x86-64*)
        listing="$(mktemp)"
        trap 'rm -f "$listing"' EXIT
        "$OBJDUMP" -d --no-show-raw-insn "$BIN" > "$listing"
        zmm="$(LC_ALL=C grep -ac '%zmm' "$listing" || true)"
        ymm="$(LC_ALL=C grep -ac '%ymm' "$listing" || true)"
        # The Rust crates alone carry AVX2 code, so no ymm means the listing is wrong
        # and the zero above proves nothing.
        if [[ "$ymm" -eq 0 ]]; then
            echo "Error: $OBJDUMP found no ymm registers in $BIN, so it did not read the code" >&2
            exit 1
        fi
        if [[ "$zmm" -ne 0 ]]; then
            echo "Error: $BIN uses AVX-512: $zmm instructions with zmm registers." >&2
            stale_hint
            echo "If a Rust crate brought them in behind a run-time CPU check, narrow this check." >&2
            exit 1
        fi
        echo "✓ $BIN uses no AVX-512 (0 zmm, $ymm ymm)"
        ;;
    *Mach-O*arm64*)
        status=0
        for feature in matmul_int8 sve sme; do
            # Each of these functions returns a constant fixed at compile time.
            body="$("$OBJDUMP" -d --no-show-raw-insn --disassemble-symbols="_ggml_cpu_has_$feature" "$BIN")"
            if ! LC_ALL=C grep -Eq 'mov[[:space:]]+w0, (#0x0|#0|wzr)([[:space:]]|$)' <<<"$body"; then
                echo "Error: $BIN does not report ggml_cpu_has_$feature as off" >&2
                status=1
            fi
        done
        if [[ "$status" -ne 0 ]]; then
            stale_hint
            exit "$status"
        fi
        echo "✓ $BIN builds ggml without i8mm, SVE and SME"
        ;;
    *)
        echo "Error: no baseline known for $BIN: $kind" >&2
        exit 1
        ;;
esac
