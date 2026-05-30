# Reversible SQLite Migrations

## Context

[`rust/migrations/`](../rust/migrations/) holds six forward-only `.sql` files.
A schema mistake ships a one-way change; the only recovery path today is
writing a compensating forward migration, which is awkward during development
when iterating on a new column.

## Goal

Allow new migrations to ship with optional down-pairs so dev iteration is
cheaper, without rewriting the existing six (they are stable and shipped).

## Sketch

- Adopt `refinery` or `rusqlite_migration` going forward; existing 001–006
  stay as-is (treat them as the baseline).
- Migration files become `NNN_name.up.sql` / `NNN_name.down.sql` for new ones;
  the runner reads both and exposes a dev-only `meeting-server --rollback-one`
  flag.
- Production startup still runs `up` only. `down` is dev-only and gated.

## Expected Outcome

Migration authors can iterate locally without manual schema surgery; release
behavior is unchanged.
