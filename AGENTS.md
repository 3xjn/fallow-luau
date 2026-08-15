# fallow-luau agent guide

Read `SPEC.md` before writing code. The feature matrix there is the product.

## Rules

- Port Fallow *formulas*, not Fallow source.
- Every function is a unit, including nested `local function`. File line count is not enough.
- Core resolves string-literal `require` only. Project-specific import wrappers are optional plugins.
- Dynamic `require` / `loadstring` are unresolved edges, not reachability.
- Skip CSS, npm, TypeScript, and Fallow Cloud.
- JSON reports include `_meta` when `--explain` or MCP is used.
- Add a fixture test for every metric. Use tiny Luau snippets in `tests/fixtures`, not a downstream app.

## Layout

```text
src/            Rust library + CLI
tests/fixtures  Luau corpora
docs/           algorithm notes (cite Fallow docs URLs)
```

## Verification

`cargo test` must stay green. Do not add a second language runtime unless SPEC says so.
