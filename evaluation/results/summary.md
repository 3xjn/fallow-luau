# Agent value pilot: observed results

**The analyzer is more correct, but this pilot did not demonstrate a net agent
productivity gain.** Both conditions produced two independently validated
repairs. The tool supplied a useful navigation hint on the SDK and a misleading
dead-code ratio on the reactive library.

## Design

Four independent GPT-6 Sol / medium agents, with no conversation history,
worked on isolated copies of two pinned public projects. Each received the
same behavior-defect repair task. The assisted condition additionally received
the frozen analyzer and instructions to use its health report. The control
used normal source reading, search, and project tools. No defects were seeded
and no suspected functions were disclosed to either condition.

The platform permits three sub-agents alongside the parent, so the last run
started when a slot became available. This was not a simultaneous timing
experiment. All work stayed local; no upstream issues, PRs, pushes, or messages
were sent.

See the [protocol](../README.md), [exact prompts](../prompts), and
[manifest](manifest.json). The analyzer binary SHA-256 was
`A6E7BEABEEE73C14F51A0FCEB2376478DC409B4AB515D1E8E9295FF0925FEB85`.
The requested model/effort are recorded; no lower-level model snapshot or
token/cost telemetry was exposed.

## Independently replayed repairs

| Project | Without tool | With tool |
| --- | --- | --- |
| PostHog Roblox SDK | Fixed stale flag values, payloads and details surviving a successful reload. | Fixed the same stale-cache defect; regression also checks omitted metadata on a retained flag. |
| alien-signals-luau | Fixed a failed initial effect retaining subscriptions and masking the callback error. | Fixed nested tracking pauses restoring a subscriber too early because nil stack entries disappear. |
| Validated local repairs | **2** | **2** |

For every submission, the evaluator created another checkout at the pinned
original commit, applied only the agent's test patch, and observed failure.
Applying the production patch made the regression and existing suite pass.
The evaluator also inspected each source diff and its behavioral contract.
These checks establish the selected fixes, not absence of all possible bugs.

| Run | Original + new regression | Patch + full suite | Evidence |
| --- | --- | --- | --- |
| PostHog control | Exit 1 | Exit 0; 49 passed; coverage 39/39 | [Replay](posthog-roblox-control/replay.json), [original](posthog-roblox-control/original-with-regression.txt), [patched](posthog-roblox-control/patched-with-regression.txt), [source](posthog-roblox-control/source.patch), [test](posthog-roblox-control/tests.patch) |
| PostHog assisted | Exit 1 | Exit 0; 49 passed; coverage 39/39 | [Replay](posthog-roblox-assisted/replay.json), [original](posthog-roblox-assisted/original-with-regression.txt), [patched](posthog-roblox-assisted/patched-with-regression.txt), [source](posthog-roblox-assisted/source.patch), [test](posthog-roblox-assisted/tests.patch) |
| Alien control | Exit 1 | Exit 0; all tests passed | [Replay](alien-signals-control/replay.json), [original](alien-signals-control/original-with-regression.txt), [patched](alien-signals-control/patched-with-regression.txt), [source](alien-signals-control/source.patch), [test](alien-signals-control/tests.patch) |
| Alien assisted | Exit 1 | Exit 0; all tests passed | [Replay](alien-signals-assisted/replay.json), [original](alien-signals-assisted/original-with-regression.txt), [patched](alien-signals-assisted/patched-with-regression.txt), [source](alien-signals-assisted/source.patch), [test](alien-signals-assisted/tests.patch) |

Adjudication note: the Alien control agent described a failure at the later
setter, but replay of its final combined regression stopped first at the
original-error assertion. An independent [cleanup probe](alien-signals-control/cleanup-probe.luau)
confirmed both claims separately: the original masks the error, runs the
callback twice, and throws from the setter; the patch preserves the original
error, runs the callback once, and lets the setter succeed. See
[original probe](alien-signals-control/original-cleanup-probe.txt) and
[patched probe](alien-signals-control/patched-cleanup-probe.txt).

## What the tool contributed

The PostHog assisted agent reported following the complexity finding for
`FlagCache.applyResponse`. Independent replay of the analyzer confirms that
finding (cyclomatic 14, cognitive 25). The control found the same bug by source
inspection after exploring queue behavior. This supports usefulness as a
navigation aid in that run, not an increase in repair yield.

The Alien assisted agent initially followed complexity findings into
`src/system.luau`, then found the pause defect by reading source and the
untracking test. The analyzer did not identify that bug. Its reported
`dead_code_ratio: 1.0` for `src/system.luau` was misleading: the library uses
`@self/system`, which the resolver does not understand. The agent recognized
this and did not delete code. See the [agent report](alien-signals-assisted/agent-report.md)
and [replayed analyzer report](alien-signals-analyzer.json).

The correct conclusion is **no demonstrated net benefit in this pilot**, not
"the tool never helps." Both conditions had successful repairs. There were
no observed suite regressions after the selected patches. Different defects,
one run per condition/project, non-blinded adjudication, and heuristic tool
coverage prevent a general causal claim. Timestamps are dispatch/completion
observations, not execution telemetry; they do not support speed or cost
comparisons. No broader quality or statistical claim is made.

## Analyzer changes verified before the runs

- Traverse returned functions and return-expression decisions.
- Recognize returned locals, indexed writes, compound-assignment reads, and
  type annotations; retain entry-module exports and exported types.
- Canonicalize paths consistently on Windows and decode long-string requires.
- Count a union of duplicated lines instead of repeatedly counting overlaps.
- Apply configured entry/ignore globs and CLI/MCP analysis thresholds.
- Share CLI/MCP health composition, including the actual export denominator.
- Use MCP newline-delimited stdio and return a failing CLI exit status for a
  failed audit.
- Give CLI analysis the existing project test stack size after real-project
  debug runs overflowed the Windows main-thread stack.
- Add regression tests, Linux/Windows test CI, and tests before release
  packaging. Mark full parity as unproven and disclose known limitations.

`cargo test --locked`: **37 passed**, with the previously failing Windows
fixture green. [Analyzer test log](analyzer-tests.txt).
Frozen-binary smoke runs succeeded on PostHog (42 files), Alien Signals
(12 files), and Roblox Signals (36 files). Roblox Signals was excluded from
patch evaluation because its full suite requires hosted Roblox execution.
No GitHub Actions runs were triggered; these checks ran locally.

Known limitations remain, including unresolved aliases and heuristic dead-code
analysis. The pilot makes resolver coverage and withholding unjustified
dead-code certainty a stronger priority than adding more commands. No further
feature expansion or repeated trials were undertaken to manufacture a positive
result.
