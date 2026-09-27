# Evaluation

## Selected behavior defect

An `effect` callback that reads a signal and throws during its initial run leaves a live subscription even though `effect` throws before it can return a stop function. A later signal update runs the failed callback again and throws from the setter. The initial failure also reports `attempt to call a string value` because the local `error` result shadows Luau's `error` function.

Affected source: `src/init.luau`, in the `effect` constructor's initial callback error path.

## Reproduction and evidence

The regression in `tests/effect.spec.luau` creates a signal, calls `effect` inside `pcall`, reads the signal in the callback, increments a counter, then raises `initial effect failed`. It checks that the caller receives that error and that changing the signal afterward does not run the callback again. This tests a public behavior: a failed creation has no disposer to call, so it must not retain a subscription.

Commands used in this checkout:

1. `Get-Content README.md` — identified the public `signal` and `effect` usage and documented test command.
2. `Get-Content src/init.luau; Get-Content src/system.luau` and `Get-Content tests/effect.spec.luau` — traced effect creation and subscription cleanup, and checked existing test conventions.
3. `C:/Users/asher/.cargo/bin/lune.exe run tests` before adding the regression — `All tests passed!` (exit 0).
4. `C:/Users/asher/.cargo/bin/lune.exe run tests` with the regression and original source — failed (exit 1). The signal setter invoked the failed callback again; the error was `src:52: attempt to call a string value` from `notifyEffect`.
5. `C:/Users/asher/.cargo/bin/lune.exe run tests` after the repair — `All tests passed!` (exit 0).

Expected: the initial error reaches the caller, and an effect whose creation failed does not respond to later signal changes. Observed before repair: the error was replaced by a string-call error, and the failed effect remained subscribed.

## Change

On initial callback failure, clear the new effect's dependency links with the existing tracking cleanup functions, then raise the captured callback error. The local result is named `err` so Luau's `error` function remains callable. Successful effect creation and the public API are unchanged. The behavior change is confined to failed initial creation: its subscriptions are removed and its original error is surfaced.

## Scope and uncertainty

This repair addresses the initial `effect` creation path. Errors during later effect notifications and failed `effectScope` creation have separate error paths and were not changed or tested here. No online issues, sibling runs, or repository history were inspected. No false lead required additional work; the source path and failing regression directly identified this defect.
