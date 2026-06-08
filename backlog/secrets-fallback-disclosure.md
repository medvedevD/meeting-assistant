# Disclose Plaintext Secrets Fallback

> **Promoted & expanded** → `plans/active/secret-storage/` (prd-v1.0 + plan-v1.0).
> That plan supersedes this note: it covers the honest `secret_storage` contract,
> the concrete keyring-mechanism disclosure, the Linux mock-keystore fix, and an
> optional passphrase vault. This slice is Phase 1–2 there.

## Context

[`KeyringSecretStore`](../rust/crates/adapters/src/secret_store.rs) falls back
to a `0600` `secrets.json` file when the OS keyring is unavailable — common on
unsigned macOS dev builds and on Linux without a running Secret Service. The
file is permission-restricted but **not encrypted**. The backend already
distinguishes the two states; the user-facing settings screen does not surface
which one is in effect.

## Goal

Users should be able to tell at a glance whether their API keys are protected
by the OS keyring or sitting in a plaintext file, and act on it if they care.

## Sketch

- Extend the sanitized `GET /api/v1/settings` payload with a top-level
  `secret_backend: "keyring" | "file"` field (plus optional file path when
  `file`).
- Settings screen shows a small banner / chip near the API-key inputs that
  reflects the state; link to docs explaining what the fallback means.
- No change to write semantics — the existing fallback behavior stays.

## Expected Outcome

Security-conscious users can see exactly where their keys live; everyone else
sees a neutral chip and is unaffected.
