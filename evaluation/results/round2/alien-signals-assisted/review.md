# Production review

Reviewed `src/init.luau` and `src/system.luau`, the complete production source in this checkout. I read `README.md` and all six existing spec files through `tests/init.luau`. I left production source unchanged. Reproductions are in `tests/review_repro.luau`; they are deliberately outside the baseline test entry point so the observed defects do not turn the existing suite red during review.

## Evidence and commands

Run from this checkout:

```powershell
& 'C:/Users/asher/Documents/ChatGPT/fallow-luau/target/evaluation/round2/bin/fallow-luau.exe' health --root . --complexity --file-scores --explain
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/review_repro
```

The baseline said `All tests passed!`. The analyzer analyzed 12 Luau files and flagged `createReactiveSystem/propagate`, `/checkDirty`, and `/clearTracking` for complexity. That was useful for directing inspection of the graph engine. Its complexity and estimated CRAP scores did not establish a defect or a worthwhile extraction; the verified problems below are in the public API layer. The analyzer's static estimated coverage is not measured test coverage. Existing topology and effect tests exercise many graph paths, but do not exercise the cases below.

Repro output on unchanged source:

```text
initial effect error true false | ...\src:204: attempt to call a string value
computed error true false | ...\src:86: ...\tests/review_repro:19: computed failure
rerun effect error true false | ...\src:52: attempt to call a string value
scope error true false | ...\src:219: attempt to call a string value
nested pause true 2
nested scope disposal true 2
direct scope disposal control true 1
```

The first `true` in each row means the reproduction harness itself completed. Each error row's `false` is the result of the inner `pcall`. The computed case is a control: it preserves the callback's failure message.

## Behavior defects

1. **Effect and scope callback errors lose the original failure.** `notifyEffect` (line 42), `effect` (line 189), and `effectScope` (line 209) in `src/init.luau` bind the second `pcall` result to a local named `error`, then call `error(error)` (lines 52, 204, 219). The local string shadows the global error function. Initial effect, rerun effect, and scope failures therefore surface `attempt to call a string value` instead of `effect failure`, `rerun failure`, or `scope failure`. This breaks diagnosis and any caller handling errors by message. The smallest change is to rename the local result (for example, `err`) and call the global `error(err)`. Compatibility risk: callers that mistakenly depend on the accidental replacement message would see the real error; a traceback level may change. The computed control and `tests/topology.spec.luau` error tests justify preserving callback failures.

2. **Nested `pauseTracking` resumes tracking too early.** `pauseTracking` and `resumeTracking` (lines 167–174) use an array of `Subscriber?`, but `table.insert(pauseStack, activeSub)` does not preserve a nil slot when the second pause occurs while tracking is already paused. The first resume pops the original subscriber, so a signal read between the two resumes becomes a dependency. In `tests/review_repro.luau`, the effect runs twice after `setValue(1)` although the read occurs while the outer pause remains active; it should run once. The smallest change is to store an explicit sentinel or a stack frame with a field that can hold nil, then restore one frame per resume. Compatibility risk: code relying on premature subscription will stop receiving those notifications. `README.md` exposes pause/resume through the package, and `tests/untrack.spec.luau` establishes that reads while paused should not subscribe.

3. **Stopping an outer scope leaves a nested scope's effects active.** `effectScope` (lines 209–221) sets `activeScope = e` for the body but never links the new scope to the previous scope. Direct child effects link to the outer scope and stop correctly; a nested scope's effects link only to that nested scope. The repro's nested effect runs twice after stopping the outer scope, while the direct child control runs once. Expected behavior is that stopping a scope disposes its descendant effects, consistent with the documented scope behavior and the direct child test in `tests/effectScope.spec.luau`. The smallest change is to link a newly created scope to the enclosing active scope before entering its body, with characterization for nested disposal and independent stopping of the inner scope. Compatibility risk: code that has been treating nested scopes as independent lifetimes would see their effects stop with the outer scope. This expectation is inferred from scope nesting rather than explicitly specified in the README; confirm intended nesting semantics during adjudication.

## Discarded leads and gaps

- The analyzer's large `propagate` and `checkDirty` functions suggested a possible refactor, but source inspection and the existing topology tests did not establish a specific maintenance failure or a smaller design with equal graph behavior. A lower complexity score alone is not a reason to change them.
- `updateComputed` also catches callback errors, but its local is `newValue`; the executable control shows the original error survives. It is not part of finding 1.
- I did not measure memory retention, performance, or every reactive graph topology. No claim about those is made here.
