# Agent value pilot

Question: does access to fallow-luau help an agent find a real issue and make a
safe improvement in an unfamiliar Luau codebase?

Latest: [overall comparison and independent replay evidence](results/comparison.md).
The original pilot below is preserved. The user's request for more and better
suggestions led to [round 2 full reviews](round2.md) and a
[round 3 fresh-project comparison](round3.md), including refactor proposals.

This is an exploratory paired comparison, not a statistical estimate. The
smallest plural-codebase comparison uses an SDK and a reactive library, with
one independent run per condition on each project. No score or improvement
threshold is assumed. A null or negative result is an acceptable outcome.

## Pinned inputs

| Project | Commit | Baseline command | Preflight |
| --- | --- | --- | --- |
| PostHog/posthog-roblox | `4f722e896baeb92ce2ed0b369963dbc512115922` | `lune run tests/runTests.luau` | 48 passed; function coverage 39/39 |
| Nicell/alien-signals-luau | `db7a368803c74cf54497b6fdaea03e4debf956bf` | `lune run tests` | All tests passed |

Runtime: Lune 0.10.4, available locally and matching PostHog's pinned runtime.
Alien Signals pins an older Lune; the same available runtime is used for both
conditions and its original suite passes. This limits version-specific claims.

Roblox/signals at `7ef2ff7db01f6955cf7d9e5a0becb3129f7f8d60` is an additional
analysis smoke test. Its full suite needs Roblox hosted execution credentials,
so it is excluded from the safe-patch comparison. It is not a failed agent run.

## Conditions

Each run gets an isolated checkout, the same task below, the same model and
reasoning setting, and ordinary source/search/shell tools. Runs start without
this conversation or other agents' findings. The treatment adds a frozen
fallow-luau binary and its documented commands. The control must not use the
binary, its source, reports, or another run's artifacts. These are harness
instructions, not an OS sandbox; report any observed contamination.

Do not give either condition suspected bugs or preferred target functions. Do
not use online issues or commit history to find known fixes. Do not coach an
agent after seeing its results. Preserve unsuccessful and no-change runs.
All third-party changes stay in local experimental copies; no upstream push,
issue, PR, or publication is authorized.

### Common task

Find a reproducible behavior defect in production Luau code in this checkout
and make the smallest safe repair. Read the local documentation and run the
existing test command. Choose a concrete behavior with evidence, not a style
preference or an improvement to a health score. Add a regression that fails
against the original code and passes against your fix, then run the original
suite. Keep the public API compatible unless correcting the defect requires
otherwise, and explain any behavior change. Do not weaken or remove tests.
If you cannot substantiate a safe change, report that rather than manufacture
one. Do not inspect sibling checkouts, evaluation results, online issues, or
git history. You are not alone in the workspace: edit only your assigned
checkout and do not revert anyone else's work. Do not delegate further or
contact third parties.

Write `EVALUATION.md` in your checkout with the symptom, affected source,
reproduction, expected and observed behavior, change, before/after regression
results, full-suite result, and uncertainties. Record the commands used and
explain which evidence led you to the issue. Report any false leads. Stop when
the selected repair and its evidence are complete, or explain why no supported
repair was found.

### Treatment addition

You have fallow-luau at the supplied absolute binary path. Start with its
`health --root . --complexity --file-scores --explain` report and use `inspect`
or `list` when relevant. Treat metrics as navigation hints, not proof of a bug
or permission to delete code. Imports outside literal filesystem paths are
unresolved; dead-code analysis is heuristic. Verify any hypothesis in source
and by executing tests. Record whether the tool changed your investigation
and which findings were useful or misleading. Do not read the analyzer's source.

### Control addition

Use ordinary source reading, search, and the project's existing tools. Do not
use fallow-luau or read its source or reports.

## Evidence and adjudication

Keep project/tool commits or hashes, prompts, agent identifiers, model/harness
settings exposed by the platform, start/end times, diffs, regression output,
and original-suite output. Missing token or cost telemetry stays unavailable;
do not infer savings from wall time. No new time, token, or retry budget is
imposed by this pilot.

Independently replay the candidate regression on the pinned original and the
patched checkout. A validated patch needs an observed original failure,
patched success, passing original suite, and source review for compatibility
and changed behavior. Distinguish test improvements from production fixes.
Review every submitted candidate, including failures. Attribute candidates
to their actual symptoms so duplicate findings are not counted twice within
a run.

Compare confirmed defects, validated patches, regressions introduced, and
unsupported claims. Report elapsed time descriptively and tool false leads.
The evaluator knows the conditions; this pilot is not blinded. Different
discovered defects and stochastic agents limit causal conclusions. A larger
randomized repeat study would be needed to estimate a general productivity
effect; this pilot does not claim one.
