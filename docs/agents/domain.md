# Domain docs

This repository uses a single-context domain documentation layout. Engineering skills use the following rules when exploring the codebase.

## Before exploring, read these files

Read the domain documentation relevant to the area you're about to change:

- Read `CONTEXT.md` at the repo root.
- Read ADRs under `docs/adr/` that affect the area.

If these files don't exist, proceed silently. Don't flag their absence or suggest creating them upfront. The `/domain-modeling` skill, reached through `/grill-with-docs` and `/improve-codebase-architecture`, creates them lazily when terms or decisions are resolved.

## File structure

This repository uses one root context and one shared ADR directory:

```text
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

## Use the glossary's vocabulary

When your output names a domain concept in an issue title, refactor proposal, hypothesis, or test name, use the term defined in `CONTEXT.md`. Don't drift to synonyms that the glossary explicitly avoids.

If the concept isn't in the glossary, reconsider whether you're inventing language the project doesn't use. If the gap is real, note it for `/domain-modeling`.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface the conflict explicitly instead of silently overriding it:

> _Contradicts ADR-0007 (event-sourced orders), but worth reopening because…_
