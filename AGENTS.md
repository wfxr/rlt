# Repository guidelines

`rlt` is a reusable Rust load-testing framework. Favor measurement accuracy, stable user-facing contracts, and predictable automation over local implementation convenience.

## Behavioral invariants

- Treat benchmark phase boundaries as part of measurement correctness. Complete setup and warmup across workers before measured execution starts, exclude warmup results and paused time from measurements, and preserve the ordering of phase transitions.
- Apply run-level controls globally across workers. Concurrency must not multiply iteration or rate limits, and cancellation must stop pending work promptly while preserving cleanup.
- Keep measurement independent of presentation. Collector choice and progress rendering must not change report data, progress delivery remains best-effort, and TUI paths restore terminal state on every exit path.
- Treat the public Rust API, CLI arguments, exit behavior, output channels, and serialized report formats as compatibility surfaces. Make breaking changes explicit and update their documentation and tests together.
- Keep optional features additive and valid in supported combinations. Code must not assume default features are enabled; unavailable feature-dependent behavior must fail with a clear configuration error.
- Preserve failure semantics across the run lifecycle. Configuration, setup, and reporting failures fail the run; iteration failures remain observable in report data; cleanup failures during cancellation must not hide the primary result.
- Protect baseline integrity and comparability. Validate compatibility before starting a benchmark, compare before replacing a baseline, write baseline data atomically, and avoid persisting sensitive command-line input. For changes to baseline storage, comparison, schemas, or regression behavior, read `docs/rfcs/0001-baseline-comparison.md`.

## Verification

A behavior change is incomplete until validation covers the affected contract and its meaningful failure paths.

- Test externally observable behavior instead of implementation details.
- Use explicit synchronization in concurrent and timing-sensitive tests. Cover phase ordering, global limits, pause behavior, cancellation, and cleanup without relying on fragile sleeps.
- For CLI, reporter, and baseline changes, verify output channels, exit behavior, serialization compatibility, and failure handling.
- For feature-gated changes, validate the affected feature combinations. For terminal-facing changes, cover relevant platform behavior.
- Run the checks relevant to the change as defined by the repository's CI workflows; treat those workflows as the source of truth rather than copying their commands here.

## Agent skills

### Issue tracker

Issues and specs are tracked in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

This repository uses the five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

This repository uses a single-context domain documentation layout. See `docs/agents/domain.md`.
