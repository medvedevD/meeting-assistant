#!/usr/bin/env bash
# M3 single-source-of-truth guard (Qt-migration section-08).
#
# The IPC protocol version is defined ONCE, in Rust
# (rust/crates/api/src/lib.rs `PROTOCOL_VERSION`). The Qt client's
# `kClientProtocol` is *generated* from it by qt-app/cmake/GenClientProtocol.cmake
# at build time. This check runs that exact generator and asserts the emitted
# C++ constant equals the Rust constant — so a hand-edit of either side, or a
# regression in the generator, fails CI instead of silently shipping a client
# that mis-negotiates the protocol.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_LIB="$ROOT/rust/crates/api/src/lib.rs"
GEN_CMAKE="$ROOT/qt-app/cmake/GenClientProtocol.cmake"

[ -f "$RUST_LIB" ]   || { echo "::error::missing $RUST_LIB"; exit 1; }
[ -f "$GEN_CMAKE" ]  || { echo "::error::missing $GEN_CMAKE"; exit 1; }

# 1. The Rust source of truth.
rust_ver="$(sed -nE 's/.*const +PROTOCOL_VERSION: *u32 *= *([0-9]+).*/\1/p' "$RUST_LIB" | head -1)"
if [ -z "$rust_ver" ]; then
    echo "::error::could not parse Rust PROTOCOL_VERSION from $RUST_LIB"
    exit 1
fi

# 2. The generated C++ constant — produced by the real build-time generator.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
out="$tmp/ClientProtocol.h"
cmake -DRUST_LIB="$RUST_LIB" -DOUT="$out" -P "$GEN_CMAKE" >/dev/null
cpp_ver="$(sed -nE 's/.*kClientProtocol *= *([0-9]+).*/\1/p' "$out" | head -1)"
if [ -z "$cpp_ver" ]; then
    echo "::error::generator did not emit kClientProtocol into $out"
    exit 1
fi

echo "Rust PROTOCOL_VERSION = $rust_ver"
echo "C++  kClientProtocol  = $cpp_ver  (generated)"

if [ "$rust_ver" != "$cpp_ver" ]; then
    echo "::error::PROTOCOL_VERSION single-source-of-truth broken: Rust=$rust_ver but generated C++ kClientProtocol=$cpp_ver"
    exit 1
fi

echo "✓ protocol-version single source of truth holds (= $rust_ver)"
