# fallow-luau

Codebase intelligence for Luau. Same job [Fallow](https://github.com/fallow-rs/fallow) does for TypeScript and JavaScript: unused code, duplication, complexity, git-churn hotspots, architecture boundaries, and a deterministic report agents can act on.

This is a port of Fallow's published algorithms and CLI/MCP contracts. It is not a fork of Fallow's Oxc parser and not a linter. Fallow stays JS/TS. This binary parses Luau with [`full_moon`](https://crates.io/crates/full_moon).

## Commands

```text
fallow-luau schema
fallow-luau list --root .
fallow-luau health --root . --explain
fallow-luau health --hotspots --targets
```

JSON first. Pass `--explain` to include `_meta` metric definitions so an agent does not need a second tool to know what a score means.

## Health formulas

See [`docs/health.md`](docs/health.md) and [Fallow health explained](https://docs.fallow.tools/explanations/health).

## Status

Step 1–2 of `SPEC.md`: parse, string-literal require graph, `list` / `schema`, and `health` (cyclomatic, cognitive, density, MI, SIG unit-size, hotspots, targets).

See `SPEC.md` for what is ported, adapted, or skipped.
