# Integration Notes

Source: `reviews/self-review.md` (substitute review — no external LLM was
available; disclosed honestly). I authored both plan and review, so confidence
is lower than a true external pass; nonetheless every finding is concrete and
cheap to fix without touching any locked decision.

## Integrating ALL findings (none rejected)

| ID | Finding | Action in claude-plan.md |
|---|---|---|
| H1 | WAV `file_size - 44` is wrong for hound float WAV (`fact` chunk) | Section 5 rewritten: parse RIFF chunk list, locate `data` offset, size = `file_size - data_offset` truncated to whole frame; verify against a real hound f32 file first |
| H2 | No testing/CI strategy | New **Section 7 — Testing & CI** (contract/integration tests + 3-OS build matrix); per-section acceptance tightened |
| H3 | Compose ViewModel logic unaccounted | Section 3 gains an explicit VM-audit step (map MeetingListVM/RecordingVM/ProtocolGenerateVM logic → core/API vs Qt client) |
| M1 | macOS mixed-audio clock drift hand-waved | Section 4 gains explicit clock-alignment/resampling requirement + a spike before relying on the Linux approach |
| M2 | "add a screen output" privacy/scope hazard | Section 4 made prescriptive: `minimumFrameInterval` workaround only, never retain/process screen frames |
| M3 | Protocol-version has no single source of truth | Section 1/2: generate the C++ `kClientProtocol` from the Rust `PROTOCOL_VERSION` (or CI equality check) |
| M4 | macOS packaging riskiest, no early spike | Section 6 + Appendix B: an early end-to-end macOS packaging spike on a clean 13/15 machine before Sections 3–5 go deep |
| L1 | PID polling has reuse race | Section 1 reaping: prefer an inherited pipe that EOFs on parent death (POSIX); Job Object on Windows |
| L2 | stdout token ordering hazard | Section 1: logs to stderr only; stdout reserved for the single handshake line (first byte) |
| L3 | Markdown protocol rendering under-scoped | Section 3: markdown rendering promoted to its own acceptance bullet with explicit scope |
| L4 | qt-app location/build entrypoint unclear | Section 2: state the top-level build entrypoint that replaces `run-compose.sh` |

No findings rejected — all are within the engineering scope of the plan, none
conflict with the 11 locked decisions or the interview resolutions.