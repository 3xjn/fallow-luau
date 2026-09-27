# Evaluation

## Symptom and source

Nested `pauseTracking()` calls do not keep tracking paused until the matching outer `resumeTracking()`. In `src/init.luau`, `pauseTracking()` used `table.insert(pauseStack, activeSub)`. The second pause stores `nil`, which does not preserve a stack slot. The first resume consequently restores the subscriber saved by the outer pause. A signal read between the two resumes is then tracked.

## Reproduction and expected behavior

`tests/untrack.spec.luau` now creates a computed getter that pauses twice, resumes once, reads a signal, and resumes again. After the signal changes, the computed should retain its cached value and its getter should have run once because the read occurred while tracking was paused. With the original implementation, the assertion at line 46 failed: the signal became a dependency and the computed reran.

## Change

`src/init.luau` now tracks pause depth separately from table contents. This preserves a nested pause even when the saved subscriber is `nil`. The public API is unchanged. The behavior change is that a read remains untracked until all enclosing pauses have been resumed.

## Evidence and commands

All commands ran in this checkout:

1. `C:\Users\asher\Documents\ChatGPT\fallow-luau\target\evaluation\bin\fallow-luau.exe health --root . --complexity --file-scores --explain`
2. `rg --files -g '!target'`
3. Read `README.md`, `src/system.luau`, `src/init.luau`, `tests/init.luau`, `tests/untrack.spec.luau`, and `tests/effect.spec.luau` using `Get-Content -LiteralPath ... -Raw`.
4. `C:\Users\asher\.cargo\bin\lune.exe run tests` before changes: `All tests passed!`
5. Added the regression, then ran the same test command before the source change: exit 1, `shouldKeepTrackingPausedUntilEveryNestedPauseIsResumed` failed at `tests/untrack.spec.luau:46`.
6. Applied the source change, then ran the same test command: exit 0, `All tests passed!`
7. `git diff -- src/init.luau tests/untrack.spec.luau` confirmed the change is limited to the pause stack and its regression.

The analyzer report guided an initial look at `src/system.luau`, where it flagged complex propagation and dirty-check functions. It did not identify this defect. Its `dead_code_ratio: 1.0` for that file was misleading because the project imports modules through paths the analyzer does not resolve. The existing untracking test and the `pauseTracking()` stack implementation led to the concrete issue. No other behavior hypothesis was substantiated; the complexity scores were not treated as defects.

## Uncertainties

The regression covers balanced nested pauses in a computed getter. Behavior for unmatched `resumeTracking()` calls is outside this repair; the API does not document such calls.
