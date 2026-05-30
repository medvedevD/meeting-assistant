# Port-Fake Compile-Time Guard

## Context

The `fakes` feature on [`meeting-core`](../rust/crates/core/src/fakes.rs)
provides test doubles for every port. Adapter and API tests reach for them,
but there is no compile-time contract that the fakes actually implement the
current port traits — a port-signature change can leave a fake stale and only
the affected test detects it (sometimes much later).

## Goal

Catch drift between port traits and their fakes at compile time, not at the
next test that happens to touch the changed method.

## Sketch

- In `meeting-adapters` (or a new `tests/` integration crate), add a
  `#[cfg(test)]` module that constructs each fake behind its port trait
  object: `let _: Arc<dyn Transcriber> = FakeTranscriber::new(...);` and so on
  for every port.
- The module compiles only if every fake still satisfies its port; a missing
  method or signature change becomes a build error immediately.
- Zero runtime cost; lives entirely in test builds.

## Expected Outcome

Adding a method to a port trait fails the test build until the matching fake
is updated, surfacing the contract change at the right moment.
