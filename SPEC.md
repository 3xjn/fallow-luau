# fallow-luau spec

Port [Fallow](https://github.com/fallow-rs/fallow) health, graph, and agent surfaces to Luau. Algorithm source: [docs.fallow.tools/explanations/health](https://docs.fallow.tools/explanations/health).

Do not invent new scoring. Copy the published formulas. Adapt only how Luau modules connect.

## Stack

- Rust CLI + library
- Parser: `full_moon` (Luau)
- Deterministic JSON envelopes; no model inside the analyzer
- MCP server that wraps the same library as the CLI

## Port (same math)

| Surface | Notes |
|---|---|
| Cyclomatic / cognitive / density | Every function is a unit, including nested `local function`. |
| Maintainability Index | `100 - (density × 30) - (dead_ratio × 20) - min(ln(fan_out+1)×4, 15)` |
| Unit size / interfacing profiles | SIG bins. Flag functions over 60 lines. |
| CRAP | `CC² × (1 − cov/100)³ + CC`. Default `static_estimated` from the test require graph. Real coverage later. |
| Hotspots | `normalized_churn × normalized_density × 100`. Churn = recency-weighted commits, 90-day half-life. |
| Trend | First half vs second half of the window: accelerating (>1.5×), cooling (<0.67×), stable. |
| Targets + effort + baselines | Same priority weights and effort rules as Fallow. |
| Duplication | Token / suffix-array clones across `.lua` / `.luau`. |
| Cycles | Require graph, no depth limit. |
| Boundaries | Configurable directory zones. |
| audit / inspect / trace / explain | Changed-file gate; symbol or function walk; rule docs. |
| watch / init / config / schema / list / suppressions / report | Same jobs as Fallow. |
| MCP + JSON `_meta` | Agents must not need a second call for metric definitions. |
| flags | Feature-flag and settings-gate detection, via plugins. |

## Adapt (Luau graph)

| Fallow JS concept | Luau equivalent |
|---|---|
| `import` / `export` | `require(...)` and keys on the returned module table |
| Entry points | Rojo `default.project.json` / sourcemap, `init.lua` / `init.luau`, configurable globs |
| Nested functions | Complexity and unused-locals walk inside any function, not only file scope |
| Tests | Files matching `**/*spec*` / `**/*test*` / `tests/**` that require production modules count as direct test references (85%) |
| Dynamic edges | Computed `require`, `loadstring`, and similar → unresolved, not fake reachability |

Resolvers are plugins. The core understands `require` with string literals. Optional resolvers may add project-specific helpers (Rojo paths, custom import wrappers). A helper that is not installed must not be assumed.

## Skip

- CSS / SCSS / Tailwind / design-system drift
- Vue / Svelte / Astro / Angular / MDX / HTML templates
- npm unused dependencies, `package.json`, TypeScript type-only imports
- TypeScript checker mode
- knip / jscpd migrate
- Fallow Cloud, beacon, paid V8 / Istanbul runtime (unless a Luau coverage format is added later)
- React / Next / Vite plugins
- Node NAPI bindings (CLI + MCP are enough)

## Build order

1. Parse + require graph + `list` + JSON schema
2. `health` (complexity, file scores, hotspots, targets)
3. `dead-code` (unused files, unused returned keys, unused locals)
4. `dupes`
5. `audit` + MCP
6. inspect / trace / explain / watch / flags / viz

## Fixtures

Ship small synthetic Luau corpora under `tests/fixtures`. Do not vendor a downstream application as the proof. A nested-function fixture must show that a 40-line inner function is scored even when the file is large.

## License / attribution

Algorithms follow Fallow's published docs. Do not copy Fallow source. The crates.io / binary name is `fallow-luau`, not `fallow`.
