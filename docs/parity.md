# Fallow → fallow-luau capability map

Reviewed against Fallow's published documentation on **2026-09-27**. We port formulas and useful workflows, not Fallow source. This is an experimental Luau adaptation, **not full behavioral parity**. `schema` labels commands `available`; that is not a parity claim.

## Verified changes in this update

| Capability | Luau behavior and proof |
|---|---|
| Function analysis | Nested functions, returned anonymous functions and branches in returns are visited. `regressions.rs` and `fixtures_health.rs`. Cognitive scoring remains an adaptation. |
| Maintainability and prioritization | Short-file dampening, project percentile normalization, effort-based ordering and the test-coverage target follow [health formulas](https://fallow.tools/docs/explanations/health/). Formula and ordering fixtures in `parity_behavior.rs`. |
| Risk evidence | CRAP findings at the configured threshold; explicit `static_estimated` model; target functions, export names, real cycle walks, importing files and clone siblings. Coverage is estimated per file. |
| Configured scope | Entry and ignore globs plus health/clone thresholds flow through CLI/MCP; explicit CLI health flags override config. `regressions.rs`. |
| Suppressions | Parser-recognized line comments suppress dead-code, health, clone and flag findings. Multiple kinds and reasons are accepted; strings containing markers are ignored. Raw function metrics remain visible. `parity_behavior.rs`. |
| Architecture zones | Resolved imports crossing disallowed zones produce `boundary_violation`; malformed globs and unknown/duplicate zone names fail. Same-zone imports are permitted; unzoned files are not rejected. `parity_behavior.rs`. |
| Changed-file audits | Analyze the full graph and clone corpus before scoping findings. Include untracked sources and clones shared with unchanged files. Scores remain full-project context. `parity_behavior.rs`. |
| New-only audit gate | Compare structural issue keys against the explicit Git ref, preserving inherited findings and exposing attribution. Comment-induced line movement does not introduce old findings. `--gate all` gates all scoped findings. Baseline extraction does not change the checkout or register worktrees. `parity_behavior.rs`. |
| Baseline errors | Missing, malformed, unsupported or unwritable health baselines return errors; save/load round trip excludes known targets. `parity_behavior.rs`. |
| Imported-key references | Dot, method, literal bracket and direct-require accesses, with local/parameter shadowing. Whole-module escapes retain exports conservatively. `trace --key` includes exact observed source lines for direct references. `parity_behavior.rs`. |
| Inspection | Method spelling and exported constants accepted; unknown symbols and ignored files return errors rather than unrelated evidence or recursion. `parity_behavior.rs`. |
| Clones | Parser tokens ignore long comments and normalize string/number literals. Covered lines use a union. Suppressed instances are excluded. `parity_behavior.rs` and `regressions.rs`. |
| CLI/MCP | Shared health composition and newline JSON-RPC transport; equality regression. Audit emits its report and exits nonzero for `fail`. `regressions.rs`. |

Tests are in [`tests`](../tests); small source examples are in [`tests/fixtures`](../tests/fixtures). Test success establishes these behaviors, not general agent productivity.

## Useful gaps still open

These are explicit gaps, not silently classified as done:

| Gap | Why it matters / current substitute |
|---|---|
| Full lexical and value-flow analysis | Unused locals/types still use name-based approximations. Module-table mutation, control-flow assignments, re-exports and indirect consumers are incomplete. Findings require consumer inspection before deletion. |
| Complete symbol call graph | [Fallow trace](https://fallow.tools/docs/cli/trace/) follows finer-grained relationships. Here only direct imported literal-key references are precise; transitive callers and all callees are module dependencies. Same-module calls are not resolved. |
| Roblox/Rojo/alias adapters | Core only resolves literal filesystem, dotted, `@self` and `init` paths. Instance-based requires and custom wrappers stay unresolved; this can materially reduce usefulness in Roblox projects. No plugin installation is implied. |
| Rules and suppression hygiene | No error/warn/off configuration, per-file health threshold overrides, stale-known-marker detection or unknown-kind diagnostics. Use entry/ignore globs and known inline kinds; inspect the suppression inventory. See [suppression docs](https://fallow.tools/docs/configuration/suppression/). |
| Audit attribution refinements | Comparison is against the explicit ref, without automatic base discovery, rename mapping, supplied-diff support or added-line clone demotion. Clone restructuring may still produce an introduced warning. See [audit docs](https://fallow.tools/docs/cli/audit/). |
| Review and impact workflows | No dedicated impact, review-brief, decision-surface or error-trace commands. Current agents compose list, health targets, inspect and trace; no equivalence claim. |
| Complete clone analysis | Only the current normalized-literal token mode; no strict/renamed-identifier/semantic modes, clone-family triage or performance claim for large corpora. See [duplication docs](https://fallow.tools/docs/cli/dupes/). |
| Full health parity | No module-scope complexity population or exact per-function test attribution. Cognitive rules and all aggregate metrics have not been exhaustively checked against Fallow. |
| Watch/config changes and output formats | Polling watch tracks source mtimes, not every analysis input. Saved reports support JSON/compact/Markdown, not Fallow's complete SARIF/CI formatter set. |

These gaps prevent a claim of “no useful parity gaps.” The implemented fixes address concrete failures in the existing product; the remaining capabilities require further implementation and evidence.

## Deliberate scope exclusions

Per [`SPEC.md`](../SPEC.md), CSS/framework analysis, npm dependency checking, TypeScript checking, Fallow Cloud, Node bindings and JS runtime integrations are excluded. Runtime coverage needs a Luau coverage format before it can be supported. Automatic code removal is not implemented; static candidates alone are insufficient proof of a safe fix.

## Configuration examples

```json
{
  "entry": ["src/main.luau"],
  "ignore": ["vendor/**"],
  "health": {"max_cyclomatic": 20, "max_cognitive": 15, "max_crap": 30, "max_unit_size": 60},
  "boundaries": [
    {"name": "core", "paths": ["src/core/**"], "allow_imports_from": []},
    {"name": "ui", "paths": ["src/ui/**"], "allow_imports_from": ["core"]}
  ]
}
```

Zone names and paths above are examples, not default project policy. An empty allow-list permits same-zone imports but denies imports into other matched zones.

```lua
-- fallow-ignore-next-line unused-local -- compatibility placeholder
local retained = 1

-- fallow-luau-ignore-file complexity, coverage-gaps -- generated state machine
```

Accepted kinds include `unused-file`, `unused-export`, `unused-local`, `unused-type`, `circular-dependency` (file scope), `boundary-violation`, `complexity`, individual complexity rule IDs, `coverage-gaps`, `large-function`, `code-duplication`, `feature-flag`, and `all`. File-wide markers apply regardless of their position. A next-line marker addresses exactly the next source line. Suppressions affect findings, not raw function complexity or size profiles.

```bash
fallow-luau audit --changed-since HEAD --gate new-only --explain
fallow-luau audit --changed-since main --gate all
fallow-luau trace src/module.luau --key create --depth 2
```

Without `--changed-since`, audit reports and gates the whole project. With it, `new-only` is the default. Findings stay in the report even when inherited; `attribution` determines which findings affect the verdict. An unreadable or invalid base fails rather than silently passing.

## Agent-value evidence

The [comparison report](../evaluation/results/comparison.md) records **9 validated repairs with the tool versus 7 without** across three broader public-project reviews, with mixed per-project results. Those runs used frozen binaries recorded in their manifests. They predate this parity update: do not present them as a new measurement of these changes. Refactoring-only gain, cost reduction and broad causal improvement remain unproven.
