# Implementation of adjudicated review findings

Only the three accepted behavior defects in `REVIEW.md` were changed. The original review and its standalone reproduction remain in this checkout. No upstream contribution was made.

## Changes

- `src/Server/FlagCache.luau`: `applyResponse` now clears old values, payloads, and details before applying each successful response, so removed flags fall back to defaults. The existing non-table guard and quota handling remain; failed HTTP fetches do not call this function.
- `src/Server/EventQueue.luau`: a 413 on a single-event batch logs a drop, removes that event, and proceeds to later events. Multi-event 413 responses still halve the batch size and back off.
- `src/Server/EventQueue.luau`: after a send, the queue removes only event tables that were actually in that batch and are still queued. A newer event added during the yielding transport call is preserved. Capacity eviction still drops the oldest queued event, `count()` stays bounded by `maxQueueSize`, and surviving events retain order.
- `tests/FlagCache.spec.luau` and `tests/EventQueue.spec.luau`: added assertions for replacement of a flag snapshot, irreducible 413 progress, and success/retry/drop/clear interactions when the queue changes during a send. The retry test advances the injected clock and verifies delivery after backoff.

## Executable evidence

Run from this checkout in PowerShell:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/runTests.luau
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/reviewReproductions.luau
& 'C:/Users/asher/.cargo/bin/stylua.exe' --check src tests
git diff --check
```

Before adding the regression tests, the original suite passed **48/48** with **39/39** instrumented functions covered. With the regression tests added but production source unchanged, it reported **49 passed, 5 failed**: the flag refresh, singleton 413, overflow success, overflow drop, and clear-during-send cases failed. The retry-overflow case already retained `b,c`, so its characterization passed before the fix. The standalone review script independently printed a stale flag/payload, three identical 413 attempts with the event still queued, and loss of the newer event.

After the production changes and final retry characterization, the full suite reports **54 passed, 0 failed**, with **39/39** functions covered. `stylua --check src tests` and `git diff --check` pass. The standalone reproduction now prints `nil` for the removed flag and payload and one 413 attempt with no queued event. Its in-flight coroutine pauses during the next send, showing `a,b,c` sent and `c` still queued at that pause; the integrated tests run sends to completion and assert the final state.

## Compatibility and remaining validation

Successful flag refreshes now discard values omitted by the server response; integrations depending on stale values will observe their configured defaults. A one-event 413 now drops that event rather than retrying it forever. Queue acknowledgement compares event-table identity, which is reliable for the SDK's freshly constructed event tables; callers who manually enqueue the *same table object* more than once are outside the SDK path. No Roblox Studio or live PostHog HTTP run was available in this checkout, so wire behavior remains unverified beyond the existing injected transport tests. The three repairs make no change to the configured queue capacity or event ordering of surviving entries.
