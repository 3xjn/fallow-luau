# Round 2: more suggestions, mixed benefit

**An observed advantage on PostHog did not transfer to Alien Signals.** The
assisted SDK review yielded four validated defect repairs versus three for
control. On the reactive library, assisted yielded two versus three. The
combined review yield is six per condition. This is a descriptive comparison,
not causal proof that analyzer access produced the difference.

| Project / condition | Submitted defect categories | Verified and repaired | Policy-dependent, withheld | Justified refactors |
| --- | ---: | ---: | ---: | ---: |
| PostHog control | 3 | 3 | 0 | 0 |
| PostHog assisted | 4 | 4 | 0 | 0 |
| Alien control | 4 | 3 | 1 | 0 |
| Alien assisted | 3 | 2 | 1 | 0 |

All four patch sets passed independent replay: adding regressions to the pinned
original failed (exit 1); applying the source patch made the expanded original
suite pass (exit 0). PostHog ended at 54 control / 56 assisted passing tests;
both retained the original 39/39 function-coverage gate. Alien's full suites
passed in both conditions. No suite regression was observed. This is headless
verification, not Roblox Studio or live endpoint verification.

## Findings and adjudication

Both SDK agents found stale flag snapshots and loss of newer unsent events
when a queue changes during an in-flight request. Control additionally found
irreducible single-event HTTP 413 retries. Assisted additionally found older
flag requests overwriting newer results and public `Group` properties missing
from flag targeting. The latter initially used a constructed state; it was
accepted only after an assertion test executed the actual unchanged public
`Group` path using service stubs and failed. The resulting test passes after
the repair. Request generations use latest-started semantics and superseded
calls return false; failed later requests retain the previously applied cache.

Both Alien agents found callback error shadowing and nested tracking-pause
corruption (different symptoms of the same mechanism, counted once). Control
also found orphaned subscriptions after failed effect/scope construction,
counted as one lifecycle defect. That repair is independently validated.

Two reproduced observations were withheld rather than counted as safe fixes:

- Control: delivering queued notifications during a later update after an
  exception. Local documentation does not settle drain-versus-discard semantics.
- Assisted: nested scopes have independent disposal. Local documentation does
  not establish descendant ownership; changing lifetimes would assume policy.

These are uncertain recommendations, not proven false observations. Every
submitted category is retained in its original review.

## Tool contribution and repaired false lead

The new resolver handles Luau `@self`, abstract `init` paths, and dotted module
names. The synthetic fixture had two unresolved edges with the original pilot
binary and zero with the revised binary. Lune executes the fixture successfully.
Alien now resolves all 17 imports, and `src/system.luau`'s erroneous 100% dead
ratio becomes zero. The analyzer's 38 tests passed after this change.

The assisted SDK agent again reported useful navigation from the complexity of
`FlagCache.applyResponse`. It found the additional issues while inspecting
source. It also queried `EventQueue.flush`, while the analyzer uses the actual
declaration name `EventQueue:flush`, and received an empty function result.
That is an observed usability limitation, not missing function traversal: a
file-level inspection includes the method and its anonymous callback.

The assisted Alien agent found high-complexity graph functions, but its verified
bugs were in the public API layer. Neither agent treated short constructor
clones or complexity alone as grounds to refactor. This round therefore does
not establish a refactoring-quality benefit.

## Reproduction evidence

Each folder contains the original review, original reproduction scripts/output,
implementation notes, source and test patches, and independent replay results:

- [PostHog control](posthog-roblox-control/replay.json)
- [PostHog assisted](posthog-roblox-assisted/replay.json)
- [Alien control](alien-signals-control/replay.json)
- [Alien assisted](alien-signals-assisted/replay.json)

See [protocol](../../round2.md), [prompts](../../prompts), and
[manifest](manifest.json). Four fresh GPT-6 Sol / medium agents used isolated
checkouts. The evaluator was unblinded. Projects were reused from the pilot,
and the analyzer was improved using that pilot's resolver evidence. Therefore
this was a development comparison, not held-out confirmation. Timing/cost
savings are not claimed. The initial one-repair pilot remains unchanged.
