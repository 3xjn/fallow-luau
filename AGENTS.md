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

## Cursor Cloud specific instructions

Pure Rust CLI + library — no services, database, GUI, or network to run. Standard commands (`cargo build`, `cargo test`, and the `fallow-luau` subcommands) are in `README.md` and `Cargo.toml`; test by pointing the CLI at a `tests/fixtures/*` corpus.

- Toolchain is pinned to `1.97.1` by `rust-toolchain.toml`; `rustup` selects it automatically (multiple channels are installed, so don't assume the default).
- `.cargo/config.toml` sets `RUST_MIN_STACK=8388608` because the AST walk uses deep recursion; debug-build tests can stack-overflow without it. Leave it in place.
- Only `cargo test` gates (see above). `cargo fmt --check` and `cargo clippy` currently report pre-existing findings in committed code, so treat them as informational rather than a signal you broke something.
- `health` hotspots read git churn, so run against a path inside this repo's git tree (the fixtures qualify).
