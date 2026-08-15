# fallow-luau

Codebase intelligence for Luau. Same job [Fallow](https://github.com/fallow-rs/fallow) does for TypeScript and JavaScript: unused code, duplication, complexity, git-churn hotspots, architecture boundaries, and a deterministic report agents can act on.

This is a port of Fallow's published algorithms and CLI/MCP contracts. It is not a fork of Fallow's Oxc parser and not a linter. Fallow stays JS/TS. This binary parses Luau.

## Commands

```text
fallow-luau health --hotspots
fallow-luau health --targets
fallow-luau dead-code
fallow-luau dupes
fallow-luau audit
```

JSON and MCP first. Every report can include `_meta` so an agent does not need a second tool to know what a score means.

See `SPEC.md` for what is ported, adapted, or skipped.