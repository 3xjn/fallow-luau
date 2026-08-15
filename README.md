# fallow-luau

Fallow's codebase health ideas, for Luau.

[Fallow](https://github.com/fallow-rs/fallow) tells you which TypeScript modules are complex, churning, unused, or stuck in a cycle. This is the same kind of tool for Luau: parse the tree, follow `require`, score functions, and emit JSON an agent can act on. It is not a linter and not a fork of Fallow's parser — Fallow stays on JS/TS; we use [`full_moon`](https://crates.io/crates/full_moon).

Formulas come from [Fallow's published health docs](https://docs.fallow.tools/explanations/health). We port the math, not the source.

## Installation

Same idea as Fallow: try it once, then install globally if you keep using it.

### One-shot (npx)

```bash
npx fallow-luau --version
npx fallow-luau health --root . --explain
```

### npm / pnpm / yarn (global)

Prebuilt binaries for macOS, Linux, and Windows. No Rust toolchain required.

```bash
npm install -g fallow-luau
# pnpm add -g fallow-luau
# yarn global add fallow-luau
```

The npm package uses `optionalDependencies` (`@fallow-luau/<platform>`) and falls back to downloading the matching GitHub Release asset on postinstall.

### One-liner installers

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.ps1 | iex
```

Installs into `~/.local/bin` (Unix) or `%LOCALAPPDATA%\fallow-luau\bin` (Windows) from the latest GitHub Release.

### Cargo

```bash
cargo install --git https://github.com/3xjn/fallow-luau
```

### Docker

```bash
docker build -t fallow-luau:local https://github.com/3xjn/fallow-luau.git#main
docker run --rm -v "$PWD:/workspace" --user "$(id -u):$(id -g)" fallow-luau:local health --root . --explain
```

Or with Compose: copy [`examples/docker/compose.yaml`](examples/docker/compose.yaml) into your project after building the image, then `docker compose run --rm fallow-luau health --root .`.

### GitHub Releases

Download prebuilt binaries from [GitHub Releases](https://github.com/3xjn/fallow-luau/releases) (created automatically when a new `version` in `Cargo.toml` lands on `main`).

| Asset | Platform |
| --- | --- |
| `fallow-luau-*-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `fallow-luau-*-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `fallow-luau-*-x86_64-unknown-linux-gnu.tar.gz` | Linux x64 (glibc) |
| `fallow-luau-*-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 (glibc) |
| `fallow-luau-*-x86_64-unknown-linux-musl.tar.gz` | Linux x64 (musl) |
| `fallow-luau-*-x86_64-pc-windows-msvc.zip` | Windows x64 |

### Verify

```bash
fallow-luau --version
```

## Usage

```bash
fallow-luau health --root . --explain
fallow-luau list --root .
fallow-luau schema
```

`--explain` puts metric definitions in a `_meta` object on the JSON so you (or an agent) do not need a second lookup.

Right now the 1:1 Luau surface is in place: parse, require graph, health (incl. score/baselines), dead-code, dupes, audit, explain, inspect, trace, watch, init/config, suppressions, report, flags, viz, and MCP. See [`docs/parity.md`](docs/parity.md). Scoring notes in [`docs/health.md`](docs/health.md).

## Publishing notes (maintainers)

Bump `version` in `Cargo.toml` (and run `scripts/sync-npm-version.sh` if publishing npm). Merge to `main` — the [release workflow](.github/workflows/release.yml) creates tag `v$VERSION` and uploads binaries automatically when that tag does not already exist.