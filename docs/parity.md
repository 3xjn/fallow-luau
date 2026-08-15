# Fallow → fallow-luau 1:1 map

Algorithms follow [Fallow docs](https://docs.fallow.tools/explanations/health). We port formulas and jobs, not Fallow source. Binary/crate name: `fallow-luau`.

Status: **done** | **partial** | **todo** | **skip** (SPEC).

## CLI commands

| Fallow | fallow-luau | Status | Luau notes |
|---|---|---|---|
| `schema` | `schema` | done | Capability manifest; expands as surfaces land |
| `list` | `list` | done | Files + require edges + unresolved dynamics |
| `health` | `health` | done | Same formulas; nested `local function` is a unit |
| `dead-code` | `dead-code` | todo→this PR | Unused files, returned module keys, unused locals, cycles |
| `dupes` | `dupes` | todo→this PR | Token / suffix-array clones on `.lua`/`.luau` |
| `audit` | `audit` | todo→this PR | Dead + health + dupes; changed-file gate when git available |
| `explain` | `explain` | todo→this PR | Rule docs without re-running analysis |
| `inspect` | `inspect` | todo | Compose evidence for one file / returned key |
| `trace` | `trace` | todo | Callers/callees of a returned key through require graph |
| `watch` | `watch` | todo | Re-run on file change |
| `init` | `init` | todo | Emit `fallow-luau.toml` / `.fallow-luau.json` |
| `config` | `config` | todo | Print resolved config |
| `suppressions` | `suppressions` | todo | Inventory `fallow-luau-ignore` markers |
| `report` | `report` | todo | Re-render saved JSON |
| `flags` | `flags` | todo | Feature/settings gates via plugins |
| `viz` | `viz` | todo | HTML treemap + require graph |
| `fix` | — | skip/later | Auto-remove unused keys (after dead-code is solid) |
| `mcp` / `fallow-mcp` | `mcp` | todo→this PR | stdio MCP wrapping the same library |
| CSS / npm / TS / knip / Cloud / React… | — | skip | Per SPEC |

## Health metrics (1:1 math)

| Metric | Formula / rule | Status |
|---|---|---|
| Cyclomatic | `1 +` decision points | done |
| Cognitive | SonarSource-style + nesting (Luau CF) | done |
| Density | `total_cc / lines` | done |
| Maintainability Index | `100 - dens×30 - dead×20 - min(ln(fan_out+1)×4, 15)` | done (dead wired when dead-code runs) |
| SIG unit size | 1–15 / 16–30 / 31–60 / >60 | done |
| SIG interfacing | 0–2 / 3–4 / 5–6 / 7+ | done |
| CRAP | `CC²×(1−cov/100)³+CC`; default `static_estimated` | done |
| Hotspots | `norm_churn × norm_density × 100`; 90-day half-life | done |
| Trend | accelerating / stable / cooling | done |
| Targets + effort | Same weights / effort rules | done (baselines later) |
| Health score (project letter) | Penalty table from docs | todo |
| Baselines / snapshots | `--baseline` / `--save-baseline` | todo |

## Dead code (adapted graph)

| Fallow | Luau equivalent | Status |
|---|---|---|
| Unused file | Unreachable from entry points (`init.lua`/`init.luau`, `**/main.*`, configurable) | this PR |
| Unused export | Unused key on returned module table | this PR |
| Unused type | Luau `type` / `export type` never referenced | todo |
| Unused local | Local binding never read (any nested function) | this PR |
| Unused dependency (npm) | — | skip |
| Circular import | Require-graph cycle, no depth limit | this PR |
| CSS / class members / Pinia / … | — | skip |

## Graph / resolvers

| Concept | Core | Plugin |
|---|---|---|
| `require("…")` string literal | yes | — |
| Dynamic `require` / `loadstring` | unresolved edge | — |
| Rojo paths / sourcemap | — | optional |
| Custom import wrappers | — | optional |

## MCP tools (target 1:1 names where sensible)

| Fallow MCP | fallow-luau MCP | Status |
|---|---|---|
| (schema via CLI) | `schema` | this PR |
| project / list | `list_project` | this PR |
| `check_health` | `check_health` | this PR |
| dead-code analyze | `find_dead_code` | this PR |
| `find_dupes` | `find_dupes` | this PR |
| `audit` | `audit` | this PR |
| explain | `explain` | this PR |
| CSS / runtime / fix / knip… | — | skip |

## `_meta`

Every JSON command with `--explain`, and every MCP tool response, includes `_meta` so agents do not need a second call for definitions.

## Build order (SPEC)

1. Parse + require graph + list + schema — **done**
2. health — **done**
3. dead-code — **this PR**
4. dupes — **this PR**
5. audit + MCP — **this PR**
6. inspect / trace / explain / watch / flags / viz — explain now; rest todo
