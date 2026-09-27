# Accepted repairs

Implemented the two adjudicated defects from `REVIEW.md`. Production changes are limited to `src/init.luau`; the original `tests/review_repro.luau` is preserved.

## Changes

- Renamed the `pcall` error locals in `notifyEffect`, `effect`, and `effectScope` to `err`. Their `error(err)` calls now propagate the callback's original message instead of attempting to call the caught string.
- Changed `pauseStack` entries from nullable subscriber values to records containing a nullable `sub` field. Each `pauseTracking()` now adds a real stack entry even when tracking is already paused, so each `resumeTracking()` restores exactly one level.
- Added assertion regressions to `tests/effect.spec.luau`, `tests/effectScope.spec.luau`, and `tests/untrack.spec.luau`. They check initial and rerun effect errors, scope errors, and nested pause behavior through the existing suite entry point.

## Commands and results

From this checkout, before the production edit, these three commands each exited 1 at the new assertion:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/effect.spec.luau
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/effectScope.spec.luau
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/untrack.spec.luau
```

After the production edit:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests
# exit 0: All tests passed!
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/review_repro
# exit 0: effect and scope rows contain their original failure messages;
# nested pause reports 1 run; nested scope disposal still reports 2 runs.
git diff --check
# exit 0; only Git's LF-to-CRLF working-copy warnings.
```

## Compatibility and scope

Callers that depended on the accidental `attempt to call a string value` message now receive the underlying callback error. Code that depended on premature subscriptions during nested pauses will no longer receive those updates. `error(err)` adds the current wrapper location to the message, as the existing computed error path already does.

Nested scope disposal remains unchanged. Its intended lifetime semantics are not established by local docs or tests, so the reproduced behavior in `REVIEW.md` was not used as authority for a production repair.
