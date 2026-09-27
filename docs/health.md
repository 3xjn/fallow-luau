# Health algorithm notes (fallow-luau)

Port of published Fallow health formulas to Luau. Do not copy Fallow source.

Source of truth: [Health explained](https://docs.fallow.tools/explanations/health)

## Units

Every function is a unit, including nested `local function` and anonymous
`function() ... end`. File line count alone is not a substitute.

## Formulas

| Metric | Formula |
|---|---|
| Cyclomatic | `1 +` decision points (`if`/`elseif`, loops, `and`/`or`, Luau if-expressions) |
| Cognitive | SonarSource-style increments with nesting penalties (adapted to Luau) |
| Density | `total_cyclomatic / lines` |
| Maintainability Index | `100 - density×30×min(lines/50,1) - dead_ratio×20 - min(ln(fan_out+1)×4,15)` clamped to `[0,100]` |
| CRAP | `CC² × (1 − cov/100)³ + CC` |
| Hotspot | `normalized_churn × normalized_density × 100` |
| Churn weight | `2^(-age_days / 90)` (90-day half-life) |
| Target priority | `min(density,1)×30 + hotspot_boost×25 + dead_ratio×20 + min(fan_in/P95_in,1)×15 + min(fan_out/P95_out,1)×10`; percentile floors 5 and 8 |
| Target order | Descending `efficiency = priority / effort`, with low=1, medium=2, high=3; path breaks ties |

The numeric constants above follow the [published health specification](https://fallow.tools/docs/explanations/health/), checked 2026-09-27. CRAP findings start at 30 by default (`health.max_crap` or `--max-crap`); estimates reflect file-level test reachability, not measured function coverage. Targets include supporting functions, unused export names, cycle walks, direct importing files and clone siblings. A recommendation is a place to investigate, not proof that a refactor improves behavior.

Target effort uses file size, function count and fan-in percentiles. Category precedence follows the published rules: urgent churn, highly connected cycles, high-impact files, dead exports (at least three exports), cognitive complexity, coupling outside entry points, test gaps, then other cycles. Health baselines use this project's `schema_version: 1` target-key format and are not interchangeable with Fallow snapshots. Read, parse, version and write failures return errors.

## SIG bins

- Unit size (LOC): 1–15 low, 16–30 medium, 31–60 high, >60 very high
- Unit interfacing (params): 0–2 low, 3–4 medium, 5–6 high, 7+ very high

## Require graph

Core resolves `require("...")` string literals to `.luau` / `.lua` / `init.*`.
Luau's [`@self` and abstract module paths](https://rfcs.luau.org/abstract-module-paths-and-init-dot-luau.html)
are supported: `package/init.luau` imports a child with `@self/child` and a
sibling of `package` with `./sibling`. Dots inside module names are retained.
Dynamic `require(expr)` and `loadstring` / `load` are unresolved edges, not
reachability. Rojo paths and custom import wrappers require resolver adapters that are not implemented yet.
