# Fallow → fallow-luau 1:1 map

Algorithms follow [Fallow docs](https://docs.fallow.tools/explanations/health). We port formulas and jobs, not Fallow source. Binary/crate name: `fallow-luau`.

Status: **done** | **partial** | **todo** | **skip** (SPEC).

## CLI commands

| Fallow | fallow-luau | Status | Luau notes |
|---|---|---|---|
| `schema` | `schema` | done | Capability manifest |
| `list` | `list` | done | Files + require edges + unresolved dynamics |
| `health` | `health` | done | Complexity, MI, SIG, hotspots, targets, score, baselines |
| `dead-code` | `dead-code` | done | Unused files, returned keys, locals, types, cycles |
| `dupes` | `dupes` | done | Token / suffix-array clones on `.lua`/`.luau` |
| `audit` | `audit` | done | Dead + health + dupes; `--changed-since` |
| `explain` | `explain` | done | Rule docs without re-running analysis |
| `inspect` | `inspect` | done | Compose graph/complexity/dead/dupes for one path/key |
| `trace` | `trace` | done | Bounded callers/callees on the require graph |
| `watch` | `watch` | done | mtime poll → re-run audit |
| `init` | `init` | done | `.fallow-luau.json` or `fallow-luau.toml` |
| `config` | `config` | done | Print resolved config |
| `suppressions` | `suppressions` | done | Inventory `-- fallow-luau-ignore*` markers |
| `report` | `report` | done | Re-render saved JSON (json/compact/markdown) |
| `flags` | `flags` | done | FF_/FeatureFlag/GetAttribute/settings/getenv |
| `viz` | `viz` | done | Self-contained HTML treemap + require graph |
| `fix` | — | skip/later | Auto-remove unused keys |
| `mcp` / `fallow-mcp` | `mcp` | done | stdio MCP wrapping the same library |
| CSS / npm / TS / knip / Cloud / React… | — | skip | Per SPEC |

## Health metrics (1:1 math)

| Metric | Formula / rule | Status |
|---|---|---|
| Cyclomatic | `1 +` decision points | done |
| Cognitive | SonarSource-style + nesting (Luau CF) | done |
| Density | `total_cc / lines` | done |
| Maintainability Index | `100 - dens×30 - dead×20 - min(ln(fan_out+1)×4, 15)` | done |
| SIG unit size | 1–15 / 16–30 / 31–60 / >60 | done |
| SIG interfacing | 0–2 / 3–4 / 5–6 / 7+ | done |
| CRAP | `CC²×(1−cov/100)³+CC`; default `static_estimated` | done |
| Hotspots | `norm_churn × norm_density × 100`; 90-day half-life | done |
| Trend | accelerating / stable / cooling | done |
| Targets + effort | Same weights / effort rules | done |
| Health score (project letter) | Penalty table v2; A–F grades | done |
| Baselines / snapshots | `--baseline` / `--save-baseline` on targets | done |

## Dead code (adapted graph)

| Fallow | Luau equivalent | Status |
|---|---|---|
| Unused file | Unreachable from entry points | done |
| Unused export | Unused key on returned module table | done |
| Unused type | Luau `type` / `export type` never referenced | done |
| Unused local | Local binding never read (nested included) | done |
| Unused dependency (npm) | — | skip |
| Circular import | Require-graph cycle, no depth limit | done |
| CSS / class members / Pinia / … | — | skip |

## Graph / resolvers

| Concept | Core | Plugin |
|---|---|---|
| `require("…")` string literal | yes | — |
| Dynamic `require` / `loadstring` | unresolved edge | — |
| Rojo paths / sourcemap | — | optional |
| Custom import wrappers | — | optional |

## MCP tools

| Fallow MCP | fallow-luau MCP | Status |
|---|---|---|
| schema | `schema` | done |
| list | `list_project` | done |
| `check_health` | `check_health` | done |
| dead-code | `find_dead_code` | done |
| `find_dupes` | `find_dupes` | done |
| `audit` | `audit` | done |
| explain | `explain` | done |
| inspect | `inspect` | done |
| trace | `trace` | done |
| flags | `flags` | done |
| CSS / runtime / fix / knip… | — | skip |

## `_meta`

Every JSON command with `--explain`, and every MCP tool response, includes `_meta`.

## Build order (SPEC)

1–6 all **done** for the Luau-adapted surface. Optional later: `fix`, Rojo resolver plugins, richer type-aware unused detection.
