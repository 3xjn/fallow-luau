# Observed agent value

**The broader reviews produced 9 validated defect repairs with fallow-luau
versus 7 without it.** Results vary by project; refactoring benefit and effort
savings remain unproven.

| Production review | Without tool | With tool |
| --- | ---: | ---: |
| PostHog Roblox SDK | 3 | 4 |
| Alien Signals | 3 | 2 |
| BrickBlast shared game logic, fresh input | 1 | 3 |
| Verified defect categories repaired | **7** | **9** |

These are descriptive counts from one independent agent per condition/project,
all GPT-6 Sol / medium. Findings with the same underlying mechanism count once
within a run. Agents reviewed the production scope, supplied reproductions,
then received an implementation follow-up after adjudication. No suspected bug
was supplied during review. All third-party changes stayed in local checkouts.

Every patch set was independently replayed: new tests failed against the pinned
original and passed with the patch; the expanded original suite also passed.
No suite regression was observed. The SDK suite finished at 54 control / 56
assisted tests, Alien's suites passed, and BrickBlast finished at 196 control /
198 assisted tests. Roblox service integration remains headless or source-only.

The assisted BrickBlast run added a useful profile-version safety fix: a fresh
stored record from a newer build can no longer be overwritten by older schema
reconciliation. It also fixed first-request cooldown behavior. The tool helped
agents navigate, but those extra defects were found through source review, not
direct analyzer diagnostics. This is an observed advantage in these runs;
stochastic agent variation and unblinded adjudication prevent a general causal
claim. Two projects were reused during development; only BrickBlast was fresh
after the analyzer changes. Counts are not a statistically powered estimate.

Both conditions had one policy-dependent Alien suggestion withheld. Neither
condition substantiated a refactor-only proposal. No timing, token-efficiency,
or monetary savings are claimed. These limitations must accompany any benefit
claim made from this experiment.

## Analyzer improvements and evidence

The implementation repairs return-expression/function traversal, used-type and
local-read handling, configured discovery/entries/thresholds, duplicate-line
overcounting, MCP framing and CLI consistency, audit failure exit status,
Windows path matching, long-string requires, and observed debug stack overflow.
It now also resolves Luau `@self`/`init` module paths and dotted module names.
All **38 Rust tests passed** after the resolver change.

The resolver correction removed a demonstrated false lead: Alien's live
`src/system.luau` went from a reported 100% dead ratio to zero, with all 17
literal imports resolved. Unsupported Roblox instance imports, heuristic
dead-code results, and silent empty results for a mismatched `inspect` symbol
remain limitations. No extra cleanup or refactor was justified solely by a
metric.

## All rounds retained

- [Initial one-repair pilot](summary.md): 2 validated repairs per condition.
  Its stopping rule capped repair yield, so it did not distinguish successful
  agents. It remains a tied result, not retroactively replaced.
- [Round 2 full reviews](round2/summary.md): 6 versus 6, with a per-project win
  and loss. The resolver was improved using pilot evidence.
- [Round 3 fresh-project review](round3/summary.md): 3 assisted versus 1 control,
  using the unchanged round 2 binary and review criteria.

Protocols, exact prompts, project pins, binary hashes, original reports,
reproduction scripts, patch sets and replay output are preserved under
`evaluation`. Further automatic rounds are outside the three-round fuse in
the supplied project policy; this record reports the observed gain and its
remaining uncertainty rather than treating one favorable comparison as proof
of universal benefit.
