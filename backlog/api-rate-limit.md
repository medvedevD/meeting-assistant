# Loopback API Rate Limit

## Context

The bearer-token check on `/api/*` is correct and constant-time, but there is
no rate limit. A co-tenant process on the same machine can hammer
`127.0.0.1:<port>` trying tokens; even though 256-bit entropy defeats
brute-force, there is nothing bounding the noise floor or the log volume.

## Goal

Cap the request rate per remote endpoint on the authenticated sub-router so
abusive traffic does not affect the sidecar's responsiveness or fill logs.

## Sketch

- Add `tower::limit::RateLimitLayer` (or `tower_governor`) to the `/api`
  sub-router in [`router.rs`](../rust/crates/api/src/router.rs).
- Conservative default: ~100 req/s per remote address; reject excess with
  `429 Too Many Requests` and a small jittered retry hint.
- `/health` and `/version` stay unthrottled.
- Add a test that 401 responses are also rate-limited (so a brute-force
  attempt still trips the limiter).

## Expected Outcome

A misbehaving local client cannot wedge the sidecar; logs stay readable; the
legitimate GUI is well under the threshold by design.
