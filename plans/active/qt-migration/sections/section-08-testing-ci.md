# Section 08 — Testing & CI (cross-cutting)

## Background
The goal is a polished shippable app. The highest-risk new surface — the sidecar
contract and the recording recovery pass — must be tested, and both binaries
must build on all 3 OSes. This is not optional and grows alongside sections
02–07; it gates the ship.

## Requirements
Automated tests cover the sidecar contract, the recovery pass, and a Qt boot
smoke; a 3-OS CI matrix builds both binaries and runs these tests; the
protocol-version single-source-of-truth is enforced in CI.

## Dependencies
- Requires: section-02, section-03, section-05, section-06 (tests target their
  output).
- Built incrementally alongside 02–07; blocks the ship.

## Implementation details
1. **Sidecar contract tests** (Rust integration tests against `meeting-server`):
   - Handshake line is valid JSON and the first bytes on stdout; logs are on
     stderr only.
   - `/api/*` returns 401 without the bearer token, 200 with it.
   - Bind is loopback-only.
   - `/version` present; protocol-range comparison unit-tested
     (in-range/below-min/above).
   - Parent-death: spawn with the pipe (POSIX) / Job Object (Windows), drop the
     parent, assert the child exits within the budget.
   - Singleton: a second instance exits with the distinct code.
2. **Recovery-pass tests** (section-06): record → truncate the WAV mid-stream →
   run recovery → assert valid WAV + expected frame count + a `meetings` row
   exists; idempotency (run twice = no-op); both orphan kinds (no DB row /
   unfinalized header). The RIFF parser is tested against a real
   codebase-produced hound f32 file (NOT a hard-coded 44-byte assumption).
3. **Qt client smoke** (`QT_QPA_PLATFORM=offscreen`): boot the GUI, complete the
   handshake + health gate against a real `meeting-server`, assert a meetings
   fetch round-trips; simulate a protocol-version mismatch and assert the
   blocking-dialog path.
4. **CI matrix** (GitHub Actions or equivalent) on **macOS + Linux + Windows**:
   `cargo build` of `meeting-server`, CMake build of `qt-app/`, run the Rust
   contract + recovery tests and the offscreen Qt smoke. Add a CI check
   asserting the Rust `PROTOCOL_VERSION` and the generated/declared C++
   `kClientProtocol` are equal (the M3 single-source-of-truth guard).

## Acceptance criteria
- [ ] Sidecar contract tests pass (handshake, auth, loopback, version-range,
      parent-death, singleton).
- [ ] Recovery-pass tests pass (both orphan kinds, idempotent, real-WAV parser
      test).
- [ ] Offscreen Qt smoke passes (handshake + health + fetch + version-mismatch
      dialog).
- [ ] CI is green building both binaries on macOS, Linux, Windows.
- [ ] CI enforces `PROTOCOL_VERSION` == C++ `kClientProtocol`.

## Files to create/modify
- `rust/crates/app/tests/sidecar_contract.rs`.
- Recovery tests alongside the section-06 module.
- `qt-app/tests/` (offscreen smoke).
- `.github/workflows/*` (3-OS matrix + protocol-version check).
