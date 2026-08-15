# fallow-luau

Fallow's codebase health ideas, for Luau.

[Fallow](https://github.com/fallow-rs/fallow) tells you which TypeScript modules are complex, churning, unused, or stuck in a cycle. This is the same kind of tool for Luau: parse the tree, follow `require`, score functions, and emit JSON an agent can act on. It is not a linter and not a fork of Fallow's parser — Fallow stays on JS/TS; we use [`full_moon`](https://crates.io/crates/full_moon).

Formulas come from [Fallow's published health docs](https://docs.fallow.tools/explanations/health). We port the math, not the source.

```bash
fallow-luau health --root . --explain
fallow-luau list --root .
fallow-luau schema
```

`--explain` puts metric definitions in a `_meta` object on the JSON so you (or an agent) do not need a second lookup.

Right now the 1:1 Luau surface is in place: parse, require graph, health (incl. score/baselines), dead-code, dupes, audit, explain, inspect, trace, watch, init/config, suppressions, report, flags, viz, and MCP. See [`docs/parity.md`](docs/parity.md). Scoring notes in [`docs/health.md`](docs/health.md).
