# Plan Review — SUBSTITUTE (not external)

> **Honest disclosure:** the Gepetto external-review step calls `gemini` and
> `codex` CLIs. Neither is installed on this machine (`command -v` → NOT FOUND),
> and a Claude review-subagent was avoided because the subagent quota was
> exhausted earlier this session. This is therefore an **adversarial
> self-review**, not an independent external review. Treat its confidence
> accordingly — a real second model has not seen this plan.

Findings ranked by severity.

## High

**H1 — Section 5 WAV header math is likely wrong (`file_size - 44`).**
`hound` writes float WAV as `WAVE_FORMAT_IEEE_FLOAT`, which conventionally
includes a `fact` chunk; the header is NOT a canonical 44 bytes. Reconstructing
from `file_size - 44` would corrupt every recovered file. Fix: actually parse
the RIFF chunk list, locate the `data` chunk's file offset, and set its size
from `file_size - data_offset` (truncated to a whole sample frame =
`channels * 4` bytes). Verify against a real `hound` f32 file before
implementing.

**H2 — No testing / CI strategy anywhere.** For a "polished shippable" goal the
plan has zero explicit tests for the highest-risk new surface: the sidecar
contract (handshake parse, bearer-auth 401, `/version` range gate, orphan
reaping on all 3 OSes), and the recovery pass. Add a testing section or fold
contract/integration tests into Sections 1, 2, 5 acceptance with concrete test
cases, plus a CI matrix (3 OSes) at least building both binaries.

**H3 — Compose ViewModel logic is unaccounted for.** Section 3 assumes screens
are thin over the API, but flow/business logic currently lives in Kotlin
`shared/commonMain` ViewModels (MeetingListVM, RecordingVM,
ProtocolGenerateVM), not in the Rust API. Porting only "screens" risks silently
dropping behavior. Add an explicit step: audit the VMs, decide per piece whether
logic moves into the core/API (preferred) or is reimplemented in the Qt client,
and record the mapping.

## Medium

**M1 — macOS mixed-audio clock drift is hand-waved.** "Mirror the Linux
mic+parec→ffmpeg_mix approach" understates it: SCStream (system) and cpal (mic)
run on independent clocks; over a long meeting they drift. v1 needs either
resampling/timestamp alignment or an explicit, documented acceptance of bounded
drift + a spike. Don't ship "mixed" on macOS assuming the Linux approach
transfers 1:1.

**M2 — ScreenCaptureKit "add a screen output" is a privacy/scope hazard.** The
plan says "add a screen output OR large `minimumFrameInterval`". For an
audio-only meetings app, instantiating a real screen capture (even discarded)
widens the already-bad "Screen Recording" framing and wastes resources. Make it
prescriptive: use the `minimumFrameInterval` workaround, never retain/process
screen frames; document this explicitly.

**M3 — Protocol-version sync has no single source of truth.** Rust
`PROTOCOL_VERSION` and the C++ `kClientProtocol` are hand-kept in two languages
→ drift risk that defeats the very gate in Q9. Mitigation: generate the C++
constant from the Rust value at build time (small codegen / shared header), or
at minimum a CI check that asserts they match.

**M4 — macOS packaging is the riskiest area and lacks an early spike.** Ad-hoc
sign + nested helper deep-sign + Homebrew `no_quarantine` + ScreenCaptureKit TCC
+ Sequoia Gatekeeper interact in fragile ways. Recommend an early end-to-end
spike on a clean macOS 13/15 machine *before* Sections 3–5 are deep, so a
packaging dead-end is found early, not at ship time.

## Low

**L1 — Parent-death detection: prefer a closed pipe over PID polling (POSIX).**
PID polling has a PID-reuse race. A pipe/socket inherited from the parent that
the child `read`s — EOF when the parent dies — is race-free and simpler than
`kill(pid,0)` polling. Keep Job Objects for Windows.

**L2 — Bearer token on stdout: ordering hazard.** The handshake line must be the
*first and only* thing on stdout before serving; any logging framework that
writes to stdout earlier leaks/garbles the token line. State explicitly: server
logs go to stderr only; stdout is reserved for the single handshake line.

**L3 — Markdown protocol rendering is under-scoped.** Section 3 buries "implement
fresh" markdown rendering for the Protocol view (the app's core output, and the
exact thing the old Compose markdown lib couldn't port). This deserves its own
acceptance criteria / possibly a sub-section, not a parenthetical.

**L4 — `qt-app/` location vs repo conventions.** Confirm the new Qt project sits
where build tooling/`run-compose.sh` successors expect; document how it's built
relative to the Rust workspace (the plan implies separate `cargo build` +
CMake but never states the top-level build entrypoint replacing
`run-compose.sh`).