# Secret Storage — Understandable & Honest Disclosure v1.0

Supersedes `backlog/secrets-fallback-disclosure.md` (which described only the
plaintext-disclosure slice). Companion architecture/ADRs in `plan-v1.0.md`.

## Problem

API keys for LLM providers are held by the sidecar so it can sign requests. They
live in the OS keystore via `KeyringSecretStore`, which falls back to a `0600`
plaintext `secrets.json` when no keystore is reachable (unsigned macOS dev
builds, Linux without Secret Service). Two problems today:

1. **The UI lies.** `LlmPanel.qml` unconditionally tells the user "Хранится в
   системном Keychain" even when keys are sitting in a plaintext file — exactly
   the users who care are misinformed.
2. **Linux has no real keystore.** The `keyring` crate is compiled **without** a
   Linux backend feature, so on Linux it resolves to the in-memory *mock* store.
   `probe_keyring()` round-trips against the mock and reports "keyring OK", so
   keys are neither persisted nor in Secret Service, while the app believes they
   are protected. (See `plan-v1.0.md` §ADR-014.)

There is no plaintext-free way to encrypt at rest without a root of trust
(hardware or a user passphrase); the OS keystore *is* that abstraction. When it
is unavailable, the honest options are: refuse, encrypt under a passphrase, or
store plaintext-but-disclosed.

## Goal

The user, while managing an LLM API key, always understands:

- **Where** the key is stored — one of three plain-language states, plus the
  concrete mechanism (Apple Keychain / Windows Credential Manager / Secret
  Service / KWallet).
- **Which** key is currently active (per provider, masked).
- That they can **replace / delete** it, and (in fallback) **upgrade** its
  protection.

## States surfaced to the user

| State | Plain-language label | Concrete mechanism shown | Backend |
|-------|----------------------|--------------------------|---------|
| 🔒 System safe | «Системный сейф» | Apple Keychain / Windows Credential Manager / Secret Service (GNOME Keyring) / KDE KWallet | OS keystore |
| 🔑 Passphrase | «Под паролем» | — (file path shown) | Argon2id + XChaCha20-Poly1305 vault |
| ⚠️ Plaintext | «Открыто» | — (file path shown) | `0600` `secrets.json` |
| 🔒 Locked | «Хранилище заблокировано» | — | vault, awaiting passphrase |

## Scope

**In**

- Honest, structured `secret_storage` field in `GET /api/v1/settings` (kind,
  state, concrete mechanism + optional daemon detail, path).
- Fix the misleading help text; disclosure card in the LLM panel.
- Hardened plaintext write (atomic, `0600` at creation, dir `0700`).
- Real Linux keystore backend (`sync-secret-service`) so "Secret Service" is
  truthful and keys actually persist; `probe_keyring()` returns false when no
  backend is compiled.
- Optional passphrase-vault fallback with unlock/lock/change-passphrase and
  plaintext→vault / vault→keyring migration.

**Out**

- Cloud/remote secret managers (Vault, KMS) — this is a local desktop app.
- Server-issued short-lived tokens / OAuth for providers (their APIs use static
  keys).
- Passphrase recovery — forgetting the vault passphrase loses the keys (stated
  explicitly in UI).
- Memory zeroization is a follow-up hardening item, not a blocker.

## Phases (incremental, each shippable)

1. **Honest contract + hardening.** `secret_storage` snapshot field, atomic
   `0600` write, dir `0700`, Linux `sync-secret-service` + `probe` fix, fix the
   lying help text. Closes the original backlog defect.
2. **Disclosure card.** State + concrete mechanism in the LLM panel; "open
   folder" for file backends.
3. **Passphrase vault.** Vault backend, init/unlock/lock/change-passphrase
   endpoints, dialogs.
4. **Migrations.** plaintext→vault, vault→keyring, "reset store".

## Success criteria

- On every platform, the card's wording matches reality (verified per-OS).
- No code path shows "Keychain" while keys are in a file.
- On Linux, a key saved with Secret Service running persists across restart and
  is visible in `seahorse`/`kwalletmanager`; with no Secret Service, the app
  reports plaintext/vault, not a false "system safe".
