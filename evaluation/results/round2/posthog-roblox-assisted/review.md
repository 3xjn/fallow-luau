# Production review

Scope: the checkout's production Luau modules and local documentation/tests. Production source was not changed. Four behavior defects are supported below; no refactor met the review's requirement for a concrete maintenance problem plus characterization coverage.

## Evidence and commands

Run from this checkout with `C:/Users/asher/.cargo/bin/lune.exe` and the supplied analyzer binary:

```powershell
& 'C:/Users/asher/Documents/ChatGPT/fallow-luau/target/evaluation/round2/bin/fallow-luau.exe' health --root . --complexity --file-scores --explain
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/runTests.luau
& 'C:/Users/asher/.cargo/bin/lune.exe' run tests/reviewRepros.luau
& 'C:/Users/asher/Documents/ChatGPT/fallow-luau/target/evaluation/round2/bin/fallow-luau.exe' inspect src/Server/FlagCache.luau --root . --symbol FlagCache.applyResponse --explain
& 'C:/Users/asher/Documents/ChatGPT/fallow-luau/target/evaluation/round2/bin/fallow-luau.exe' inspect src/Server/EventQueue.luau --root . --symbol EventQueue.flush --explain
```

Baseline: 48 passed, 0 failed, reported 39/39 instrumented functions covered. The baseline does not exercise the Roblox-bound modules. The isolated reproduction script executes unchanged `FlagCache`, `EventQueue`, `RetryPolicy`, and (via `luau.load` with stubbed module references) unchanged `FeatureFlags` source. Its output:

```text
flag reload retired value: true
flag reload retired payload: true
flag reload live value: false
queue sent: a,b
queue remaining: 0
flags after newer request: new
flags after older request completes: old
Group-derived flags membership: guild_42
Group-derived flags properties: nil
```

The health output usefully identified `FlagCache.applyResponse` and prompted source/test inspection. It did not flag the queue defect: `EventQueue.luau` scored 95.49 and `inspect` returned no function for the `EventQueue.flush` symbol. The latter inspect output also reported several four- or five-line constructor clones shared with `FeatureFlags`; these do not justify an extraction. Scores, static coverage, and clones were navigation hints only.

## Behavior defects

### 1. A successful flag reload preserves flags and payloads absent from the new response

Affected: `FlagCache.applyResponse` (`src/Server/FlagCache.luau:42`), `FeatureFlags.reload` (`src/Server/FeatureFlags.luau:78`), and public flag reads. `applyResponse` mutates existing maps and only clears them for `quotaLimited`. A later successful response omitting `retired` leaves both its `true` value and payload cached. The reproduction prints `retired value: true` and `retired payload: true` after a response containing only `live = false`.

Expected: a successful reload represents the current server evaluation. A flag removed from that response should become missing and reads should use the documented default. The feature flag guide says `ReloadFeatureFlags` makes flags fresh and reads of missing flags use the default. Smallest change: build values, payloads, and details as a new generation from each successful response, then replace those maps while preserving the store object and current request metadata. Characterize removed flags and payloads, and v3/v4 precedence, in `FlagCache` tests. Compatibility risk: callers who relied on implicit retention of flags omitted by `/flags` will now see the documented missing/default behavior; verify whether the endpoint can intentionally send partial responses before implementation.

### 2. Enqueue during an in-flight flush can silently discard an unsent event

Affected: `EventQueue.enqueue` (`src/Server/EventQueue.luau:44`), `_takeBatch` (`:65`), `_removeFront` (`:74`), and `flush` (`:81`). With capacity two, flush snapshots events `a,b`. While transport is in flight, enqueueing `c` drops oldest `a`; after a 200 response, `_removeFront(2)` removes the now-current `b,c`. The reproduction records only `a,b` sent and zero remaining, so `c` is lost without a send. The synchronous transport callback in the script models the same queue mutation that can occur when `RequestAsync` yields to another capture task.

Expected: a successful response removes the exact events in its sent batch, while new events survive for a later batch. The queue documentation and `tests/EventQueue.spec.luau` establish delivery on success and oldest-event drop at capacity; neither authorizes dropping a newer unsent event. Smallest change: retain stable batch identity and remove only those events on success/drop, or detach the in-flight batch from the pending queue with explicit retry restoration. Characterize enqueue during a yielding send at capacity and on retry. Compatibility risk: queue capacity accounting and oldest-drop order during in-flight requests need an explicit choice; preserve the documented bounded-memory behavior.

### 3. An older flag request can overwrite a newer completed request

Affected: `FeatureFlags.reload` (`src/Server/FeatureFlags.luau:78`) and the public reload and automatic preload paths in `src/Server/init.luau`. Two reloads may overlap because the transport yields and several API paths spawn one. The reproduction suspends the first request, completes a second with `tier = "new"`, then resumes the first with `tier = "old"`; the final cache is `old`. This remains a defect even after fixing the separate missing-flag issue.

Expected: an earlier request cannot make a subject's cache older than a later completed evaluation, especially after targeting properties change. The guide describes a successful reload as making flags fresh. Smallest change: attach a per-subject request generation and apply a response only if it belongs to the current generation (or serialize reloads while retaining the latest requested body). Characterize out-of-order completion and failure of the newer request. Compatibility risk: choosing latest-started versus latest-successful behavior affects callers waiting on `ReloadFeatureFlags`; define and preserve a truthful success return.

### 4. `Group` properties are not included in flag targeting requests

Affected: `PostHog:Group` (`src/Server/init.luau:459`) and `FeatureFlags:_buildBody` (`src/Server/FeatureFlags.luau:33`). `Group` stores membership and emits `$groupidentify` with `$group_set`, but never calls `FeatureFlags:setGroupProperties`. `_buildBody` reads only `playerState.groupPropertiesForFlags` for `group_properties`. The executable reproduction constructs the state produced by that `Group` path: `/flags` body contains `$groups.guild = "guild_42"` but `group_properties` is nil. The guide explicitly says properties set through `Group` are also used for flag targeting.

Expected: `PostHog:Group(player, "guild", "guild_42", { tier = "gold" })` supplies `tier` to subsequent `/flags` requests. Smallest change: copy nonempty `groupProperties` into the subject's group targeting properties in `Group`, with a characterization test of the public path in a Roblox stub or Studio. Compatibility risk: previously omitted group attributes would now influence evaluated flags; avoid mutating the caller's properties table and clarify whether reassignment to a new group should discard attributes from the old group.

## Coverage and discarded leads

Reviewed production files: `src/init.luau`; `src/Client/{init,Platform}.luau`; `src/Server/{init,Autocapture,Context,ErrorTracking,EventQueue,ExceptionBuilder,FeatureFlags,FlagCache,PlayerRegistry,PlayerState,RelayServer,Session,Transport}.luau`; `src/Server/Storage/{IStorage,MemoryStorage}.luau`; and `src/Shared/{Defaults,Json,Logger,RetryPolicy,Sanitize,Types,Uuid,Version}.luau`. Read the local README, API and behavior guides, baseline runner, harness, and existing unit specs. Review of Roblox service behavior is source-level and headless; no Studio integration run was available. The `Group` reproduction executes `_buildBody` from unchanged source but does not instantiate the full server entry point, so that finding additionally rests on the direct source call path.

Discarded leads: `Sanitize.clean` has high cognitive complexity but its cycle/depth behavior matched its unit tests and did not reveal a defect or a justified refactor. The quota-limited branch in `FlagCache` does clear all values as documented; the stale-cache problem occurs on an ordinary successful response. The relay's reserved-name rejection and player attribution matched the documented trust boundary in source. Small duplicate constructors and file scores alone did not meet the refactor criterion. The storage interface is currently unused by the runtime, but local docs call it a future extension seam; there is no claimed behavior defect from that fact.
