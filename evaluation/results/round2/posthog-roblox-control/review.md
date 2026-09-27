# Production review

This is a review of the unchanged production source. The only added executable file is `tests/reviewReproductions.luau`; no production module or existing test was edited. I reviewed all production files under `src/`: `init.luau`; `Client/init.luau`, `Client/Platform.luau`; `Server/init.luau`, `Autocapture.luau`, `Context.luau`, `ErrorTracking.luau`, `EventQueue.luau`, `ExceptionBuilder.luau`, `FeatureFlags.luau`, `FlagCache.luau`, `PlayerRegistry.luau`, `PlayerState.luau`, `RelayServer.luau`, `Session.luau`, `Transport.luau`, `Storage/IStorage.luau`, `Storage/MemoryStorage.luau`; `Shared/Defaults.luau`, `Json.luau`, `Logger.luau`, `RetryPolicy.luau`, `Sanitize.luau`, `Types.luau`, `Uuid.luau`, and `Version.luau`. I read the existing unit tests and local user/development documentation. No sibling checkout, parent-project file, history, online issue, or analyzer input was used.

## Commands and observations

From this checkout:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/runTests.luau
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/reviewReproductions.luau
```

Baseline: **48 passed, 0 failed**, with the project's **39/39 function coverage** gate satisfied. The review reproduction prints:

```text
flag reload removes absent flag: expected nil, actual true
flag reload removes absent payload: expected nil, actual 1
single-event 413: attempts 3 queued 1 batch size 1
in-flight overflow: sent a,b remaining  expected remaining c
```

The script imports the original production modules directly. Its transport and clock are injected through the existing `EventQueue.new` seam; it uses `coroutine.yield` to model a yielding HTTP request. The test runner does not load this script, so the baseline remains a measure of existing coverage.

## Behavior defects

### 1. Successful flag reload retains flags and payloads absent from the new response

**Affected symbols:** `FeatureFlags:reload` (`src/Server/FeatureFlags.luau:78`) and `FlagCache.applyResponse` (`src/Server/FlagCache.luau:42`). The former applies each successful response to the existing store, while the latter only writes keys from the response; it clears values only on `quotaLimited`. A flag removed in PostHog therefore remains enabled locally, and an old payload survives when a refreshed response omits it. The standalone reproduction first applies `{ old = true }` and its payload, then applies a successful empty flags response; reads still return `true` and payload version `1`.

**Expected behavior and basis:** `docs/feature-flags.md` says that after `ReloadFeatureFlags` succeeds, flags are fresh; its `IsFeatureEnabled` section says an absent flag yields the supplied default. A complete successful response should replace the previous snapshot. Keep the old snapshot on a failed request.

**Smallest proposed change:** Build the next `values`, `payloads`, and `details` maps from the successful response, then replace the store maps atomically in `applyResponse` (or construct a fresh store in `reload`). Preserve the existing v4-over-v3 precedence and quota behavior. Add a two-response regression test covering removed flag, removed payload, and retained response metadata.

**Compatibility risk:** Any consumer relying on an omitted flag retaining an old value would now see the documented default. Confirm the `/flags` response is a full snapshot rather than a delta before implementation; the current code and documentation treat reload as a full refresh, but that protocol point was not tested against a live endpoint here.

### 2. HTTP 413 for a one-event batch retries the same unsendable event indefinitely

**Affected symbols:** `EventQueue:flush` (`src/Server/EventQueue.luau:81`) and its `retry_smaller` branch (`src/Server/EventQueue.luau:110`). The branch floors the adjusted size at one and backs off regardless of the current batch size. With one event, each retry sends identical payload content. The reproduction returns 413 on three attempts: the event remains queued and the adjusted size stays at one each time.

**Expected behavior and basis:** `RetryPolicy.classify` labels 413 as `retry_smaller`, and `EventQueue` documents bounded batching/backoff. Once a batch has one event, it cannot be made smaller by batch splitting. Retrying that identical request cannot make progress, blocks events behind it, and consumes HTTP budget. A one-event 413 should be discarded with an explicit warning so later events can proceed.

**Smallest proposed change:** In the 413 branch, if `#batch == 1`, remove that event and continue the flush; otherwise halve and retry after backoff as today. Add a test with a single 413 event followed by a valid event, checking the valid event is delivered.

**Compatibility risk:** A transient server-side 413 or proxy misclassification would now lose that event. This is limited to the irreducible single-event case; larger batches retain the existing retry behavior.

### 3. Queue overflow during an in-flight batch can delete an event never sent

**Affected symbols:** `EventQueue:enqueue` (`src/Server/EventQueue.luau:44`), `_takeBatch` (`:65`), `_removeFront` (`:74`), and `flush` (`:81`). `flush` copies the front batch and yields in `transport:sendBatch`. Meanwhile `enqueue` may hit `maxQueueSize` and remove the oldest queued event, including one in the copied batch. On success, `_removeFront(#batch)` then removes the same count from the changed queue, which can include newer unsent events. With capacity two, the reproduction sends `a,b`, enqueues `c` while HTTP is in flight, and ends with an empty queue. `c` was neither sent nor retained. On a failed request the queue can instead retain only part of the original batch while a later retry sends a different set.

**Expected behavior and basis:** `docs/configuration.md` says overflow drops the oldest events; `docs/capturing-events.md` says queued events are flushed to PostHog. The capacity policy may drop old `a`, but it must not delete a newly accepted `c` as if it were in the acknowledged batch.

**Smallest proposed change:** Reserve or detach the in-flight batch before yielding, then acknowledge only those exact events on success and restore them in order on retry subject to the queue cap. An alternative is to track stable IDs for in-flight events and remove only acknowledged IDs; the detach approach looks simpler if retry ordering is handled carefully. Add a coroutine-controlled characterization/regression test for success, retry, and overflow at the capacity boundary.

**Compatibility risk:** Detaching changes what `count()` and `shouldFlush()` mean while HTTP is in progress and which old event capacity eviction chooses. Specify whether in-flight events count toward the configured capacity, and preserve the bounded-memory behavior.

## Discarded leads and coverage gaps

- `FlagCache` clearing on non-empty `quotaLimited` initially looked like unexpected data loss. The dedicated existing test and `docs/feature-flags.md` explicitly require it, so it is not a finding.
- A missing device type on `player_joined` initially looked like an ordering defect. `docs/autocapture.md` explicitly documents that the first client relay can arrive later, so it is not a finding.
- `storage` appears configured but unused by the queue. The documentation calls it an advanced provider seam and does not promise persistence of queue contents, so this review does not infer a defect from that alone.
- The existing Lune coverage gate includes pure modules, not Roblox-bound entry points, relay, HTTP transport, lifecycle, client, or error wiring. I inspected those paths but did not run them in Studio or against a live PostHog project. Findings 1–3 are demonstrated in pure production modules; end-to-end behavior and `/flags` response semantics remain the material external coverage gaps.

No refactor-only suggestion met the requirement for a concrete existing maintenance problem plus characterization evidence. The findings above are behavior defects, not metric or style recommendations.
