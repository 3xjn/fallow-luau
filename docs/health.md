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
| Maintainability Index | `100 - (density×30) - (dead_ratio×20) - min(ln(fan_out+1)×4, 15)` clamped to `[0,100]` |
| CRAP | `CC² × (1 − cov/100)³ + CC` |
| Hotspot | `normalized_churn × normalized_density × 100` |
| Churn weight | `2^(-age_days / 90)` (90-day half-life) |
| Target priority | `min(density,1)×30 + hotspot_boost×25 + dead_ratio×20 + min(fan_in/20,1)×15 + min(fan_out/30,1)×10` |

## SIG bins

- Unit size (LOC): 1–15 low, 16–30 medium, 31–60 high, >60 very high
- Unit interfacing (params): 0–2 low, 3–4 medium, 5–6 high, 7+ very high

## Require graph

Core resolves `require("...")` string literals to `.luau` / `.lua` / `init.*`.
Dynamic `require(expr)` and `loadstring` / `load` are unresolved edges, not
reachability. Rojo paths and custom import wrappers are optional plugins.
