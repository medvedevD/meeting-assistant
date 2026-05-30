# Live Progress Unification

## Context

`LiveProgress` is declared as parallel type aliases in two crates:
[`meeting-api::router`](../rust/crates/api/src/router.rs) (reader, used by
`GET /jobs/:id`) and [`meeting-adapters::worker`](../rust/crates/adapters/src/worker.rs)
(writer). Both alias the same `Arc<DashMap<String, JobProgress>>`, but each
crate re-declares the type. A schema drift on `JobProgress` would compile in
one crate and not the other only if the entity itself changes shape — and even
then nothing prevents one side from going out of sync if the alias migrates.

## Goal

Make `LiveProgress` and `JobProgress` have a single declaration site so reader
and writer cannot drift, and so future work ([[worker-concurrency-pool]],
[[job-progress-sse]], [[job-cancellation]]) has an unambiguous live table.

## Sketch

- Move the `LiveProgress` alias into `meeting-core` next to `JobProgress`
  (entity already lives there).
- Re-export from `meeting-api` and `meeting-adapters` if convenient for
  call-sites, but the type definition stays in core.
- No behavior change; mechanical refactor + a compile-time check that both
  crates resolve to the same `core::LiveProgress`.

## Expected Outcome

One source of truth for the live job-progress table. Prerequisite for the
concurrency, SSE, and cancellation tasks.
