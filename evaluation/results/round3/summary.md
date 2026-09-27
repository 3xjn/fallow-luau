# Round 3: gain on a fresh project

**Assisted found and safely repaired three verified defect categories; control
found and repaired one.** Both reviewed all 17 production modules in the scoped
BrickBlast shared-game directory. No refactor-only proposal qualified in either
condition. No proposed defect was withheld in this pair.

| Finding | Control | Assisted | Established impact |
| --- | --- | --- | --- |
| Board validator accepts fractional/NaN coordinates | Validated repair | Validated repair | Broken guard for the explicitly required integer grid; occurrence in the live UI is unmeasured. |
| Reconciliation can replace a newer-schema stored profile with an older payload | — | Validated repair | Conditional data-loss race between load and update; no claim of observed live data loss. |
| First cooldown request can be rejected before any request was accepted | — | Validated repair | Deterministic with the supported explicit clock input; startup frequency in production is unknown. |

The baseline passed 14 modules / 195 tests. Independent evaluator replay added
only each agent's test patch to another pinned original checkout and observed
exit 1. Applying the corresponding source patch produced exit 0 and the
following full-suite results:

- Control: [196 tests passed](brickblast-control/patched-with-regression.txt);
  [replay](brickblast-control/replay.json).
- Assisted: [198 tests passed](brickblast-assisted/patched-with-regression.txt);
  [replay](brickblast-assisted/replay.json).

Both original reproduction scripts were also executed independently. Each run
folder preserves its review, implementation report, source/test patches,
reproduction, and before/after output. The analyzer was unchanged from round 2;
this project was selected for its existing runnable suite and different domain,
without previously known defects or source-specific tool tuning.

## Profile safety and scope

The source review checked both production reconciliation callers, although the
server itself remained outside the executable headless scope. Both call it in
an UpdateAsync callback inside `pcall`; the receipt callback immediately uses
the returned table. Returning a new cancellation sentinel would therefore
change their contract. The repair instead raises an error before copying or
mutating inputs when the fresh stored schema is newer. The existing callers
take their failure/retry paths; a receipt is not marked fulfilled. Same-version
ledger merges remain covered and passing. No server source was changed.

Roblox's [UpdateAsync documentation](https://create.roblox.com/docs/reference/engine/classes/GlobalDataStore)
describes supplying the latest stored value and re-invoking the transform on a
write conflict. The regression establishes the pure helper's refusal and input
preservation; caller compatibility is source-reviewed. It does not establish
live DataStore/Studio behavior. Save attempts can still retry and log through
existing code; the patch does not add a new session read-only state.

## Interpretation

This pair demonstrates a higher verified review yield with analyzer access on
the new input. It does not by itself establish causality or a repeatable average
effect. The assisted agent reported using complexity rankings to inspect
HandGenerator and DeveloperTools, but the actual verified defects were found
through subsequent source/test review. It also identified unreliable zero
fan counts for unsupported Roblox instance imports. Metrics did not directly
diagnose these bugs, and neither agent substantiated a refactor-only proposal.

See [protocol](../../round3.md) and [manifest](manifest.json). Both agents used
GPT-6 Sol / medium with the same task, scope, and test command. The evaluator
was unblinded. This completes the project policy's third round; earlier mixed
results remain in the [overall comparison](../comparison.md).
