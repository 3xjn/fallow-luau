# Production Luau review

## Scope and evidence

Reviewed all 17 production modules under `src/shared/game`: `Achievements`, `Blocks`, `Config`, `DeveloperTools`, `ExperienceConfig`, `GameplayRules`, `Grid`, `HandGenerator`, `ProfilePersistence`, `SimulationHarness`, `SoloLayout`, `StageProgression`, `StageVisuals`, `Telemetry`, `UIControlRules`, `VisualQuality`, and `VisualTokens`. Read the corresponding 14 test modules, `AGENTS.md`, `docs/GAMEPLAY_RULES.md`, and `docs/VISUAL_DIRECTION.md`. The review covers the headless shared rules; client, server, and Studio behavior were not inspected.

Commands from the assigned checkout:

```powershell
& 'C:\Users\asher\Documents\ChatGPT\fallow-luau\target\evaluation\round2\bin\fallow-luau.exe' health --root src/shared/game --complexity --file-scores --explain
& 'C:\Users\asher\.cargo\bin\lune.exe' run scripts/run-tests.luau
& 'C:\Users\asher\.cargo\bin\lune.exe' run scripts/review-repros.luau
```

The baseline passed: 14/14 modules, 195 tests. The local reproduction script runs against unchanged production modules and prints the observations below. The analyzer highlighted `HandGenerator.generate` (cyclomatic 30, cognitive 46) and `DeveloperTools.applyCommand` (26, 32), which directed source and test reading but did not establish a defect. Its zero fan-in/fan-out figures are misleading for Roblox instance imports, which it cannot resolve. Its CRAP scores use estimated, not measured, coverage.

## Behavior defects

### 1. Concurrent future profile can be overwritten by an older server

**Affected symbols:** `ProfilePersistence.reconcileWithStored` at `src/shared/game/ProfilePersistence.luau:177`, and its safety contract with `ProfilePersistence.migrate` at line 69.

**Current problem:** `migrate` refuses a version newer than this build when loading, but `reconcileWithStored` does not repeat that check on the fresh value supplied to an `UpdateAsync` transform. An older server can load a version-2 profile, a newer server can write version 3, then the older server's pending update can replace the version-3 record with its version-2 payload. The `futureField` and any other new data disappear. This is a conditional cross-version race, not evidence that it has occurred in a live DataStore.

**Expected behavior and basis:** The module's own rollback-safety comment says a newer profile must be treated as a failed load and never overwritten. That rule must also hold when the newer value arrives during the update transform, after the original load.

**Smallest proposed change:** Have reconciliation detect `profileVersion(existingProfile) > ProfilePersistence.Version` and signal cancellation before copying the payload. Ensure the `UpdateAsync` caller cancels that write and leaves the session read-only. Add characterization for the version-3-in-transform case alongside the current-version ledger-merge tests.

**Compatibility risk:** A cancellation result changes the return contract of `reconcileWithStored`; the server caller must be checked during implementation. The existing same-version merge behavior must remain intact.

**Executable observation:** The reproduction prints `future profile load status: future ... 3`, then `concurrent future profile rewritten as version: 2 future field: nil`. Existing tests cover refusal at load (`ProfilePersistenceTests.luau:52`) and normal reconciliation (`:228`), but not this combination.

### 2. Board-child validator accepts coordinates that cannot name a board cell

**Affected symbol:** `UIControlRules.validateBoardGridChildren` at `src/shared/game/UIControlRules.luau:228`.

**Current problem:** The coordinate check requires numbers in the 1–8 range, but does not require finite integers. `1.5` and NaN pass; `seenCoordinates` records them as distinct strings. Replacing the `(1,1)` cell's `boardX` with either value leaves 64 buttons but no cell at `(1,1)`, yet the validator reports success.

**Expected behavior and basis:** `AGENTS.md` locks the visible board to 64 direct cell buttons, and the function's own comment promises valid unique coordinates. A cell coordinate must be an integer in `[1, Config.BoardSize]`. Existing tests assert out-of-range and duplicate rejection, establishing that this is a guard, but do not cover noninteger values.

**Smallest proposed change:** Reject nonfinite or noninteger `boardX`/`boardY` before forming the coordinate key. Characterize fractional and NaN inputs while retaining the valid-grid case.

**Compatibility risk:** Any caller currently using noninteger attributes would be rejected; such attributes cannot represent logical grid cells, so this aligns with the stated invariant. Whether malformed attributes occur in the live client was outside this review.

**Executable observation:** The reproduction prints `valid 8x8 grid: true`, then `fractional coordinate accepted: true { } 64` and `NaN coordinate accepted: true { } 64`.

### 3. A first cooldown request can be rejected without a prior request

**Affected symbols:** `UIControlRules.consumeActionCooldown` at `src/shared/game/UIControlRules.luau:213` and `canFireServerAction` at line 188.

**Current problem:** `consumeActionCooldown` substitutes `0` for an absent timestamp. With an empty timestamp table, cooldown `0.45`, and clock `0.1`, it rejects the first request because `0.1 - 0 < 0.45`. The same request at `0.6` succeeds. The function's comment says a rejected burst must not extend the window, but there was no earlier request to start one.

**Expected behavior and basis:** A cooldown should begin when a request is accepted; the first request for a key should be accepted immediately. The reproduction uses the function's explicit `now` parameter, so this is deterministic. Live reachability depends on whether a call occurs within its cooldown of the clock epoch; client/server call sites were outside scope.

**Smallest proposed change:** Treat a missing `timestamps[key]` as no cooldown, record `now`, then use `canFireServerAction` only for later requests. Add characterization for first, immediate repeated, and post-cooldown calls.

**Compatibility risk:** Requests during early startup that were accidentally blocked become accepted. No later-window behavior should change.

**Executable observation:** The reproduction prints `first request at uptime 0.1 accepted: false` and `same request at uptime 0.6 accepted: true`. Existing `UIControlRulesTests` exercises `canFireServerAction` with a previous timestamp, but not `consumeActionCooldown` with an empty table.

## Refactors and discarded leads

No refactor suggestion survived review. The analyzer's complexity ranking led to `HandGenerator.generate` and `DeveloperTools.applyCommand`; both have domain-driven branches and tests for their major behaviors. A metric or function length alone would not justify splitting them, and this review found no specific maintenance failure a smaller design would resolve.

`Grid.clearLines` looked complicated in the analyzer, but source confirms it detects rows and columns before clearing either, so intersections are counted and removed consistently with `docs/GAMEPLAY_RULES.md`. `SimulationHarness` has a placement cap and can report the cap on the exact final move; that is a bounded-simulation convention, with no evidence that it misstates a shipped run. The visual modules preserve the fixed piece palette and variable stage/energy colors required by `docs/VISUAL_DIRECTION.md`; no palette rewrite is proposed. `docs/VISUAL_DIRECTION.md` mentions a `StageProgression.paletteForStage` parallel system, but the current module has no such function, so that documentation reference is not evidence of live duplicate code.

## Coverage gaps

The existing runner explicitly selects 14 tests. `Grid`, `Blocks`, `Config`, and `ExperienceConfig` have no dedicated test modules; they are exercised indirectly. This gap is context, not a standalone refactor request. The three reproductions cover pure module behavior only. The profile race needs server-call-site and DataStore-transform verification before implementation is declared complete. The board validator and cooldown need live-call-site review to establish frequency and user impact. Studio Play checks were unavailable and are not claimed.
