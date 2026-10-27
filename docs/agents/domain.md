# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root, or
- **`CONTEXT-MAP.md`** at the repo root if it exists — it points at one `CONTEXT.md` per context. Read each one relevant to the topic.
- **`docs/adr/`** — read ADRs that touch the area you're about to work in. In multi-context repos, also check `src/<context>/docs/adr/` for context-scoped decisions.
- **`architecture.md`** at the repo root — the topology, the module boundaries, the edges between them,
  and the invariants the code enforces. Read it before deciding where a change belongs.
- **the spec for the area** — `src/specs/<boundary>.md` for the boundary you are touching; the table in
  `architecture.md` says which modules each one covers, and each lists the ADRs it owns.
- **`goal-and-requirements.md`** — when the question is whether something should exist at all, or what
  the product deliberately does not do.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates them lazily when terms or decisions actually get resolved.

## Which doc owns what

Each durable fact has exactly one home. Put a change where it belongs, and link rather than restate it:

| Doc | Owns |
|---|---|
| `goal-and-requirements.md` | What the product is, who it is for, non-goals, durable principles |
| `architecture.md` | Topology, the seams, module boundaries, the edges between siblings, invariants |
| `src/specs/*.md` | One boundary's responsibility and its allowed / forbidden reaches, plus the ADRs it owns |
| `CONTEXT.md` | The vocabulary — names for types, modules, and what the screen says |
| `docs/adr/` | One decision each: why, what it cost, what was rejected |
| `docs/measurements.md` | Every measured number |
| `.scratch/<feature>/` | One feature's plan and tickets — it expires when the work lands |

A change that overturns a principle or moves a boundary updates that spec in the same change, the
same way a resolved term updates the glossary. History belongs to git, not to these files.

## File structure

Single-context repo (most repos):

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

Multi-context repo (presence of `CONTEXT-MAP.md` at the root):

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← system-wide decisions
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← context-specific decisions
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
