# Shared game production review

## Scope and baseline

Reviewed the 17 production modules under `src/shared/game`: `Achievements`, `Blocks`, `Config`, `DeveloperTools`, `ExperienceConfig`, `GameplayRules`, `Grid`, `HandGenerator`, `ProfilePersistence`, `SimulationHarness`, `SoloLayout`, `StageProgression`, `StageVisuals`, `Telemetry`, `UIControlRules`, `VisualQuality`, and `VisualTokens`. Read the checkout's `AGENTS.md`, `docs/GAMEPLAY_RULES.md`, the relevant existing tests, and the headless test runner. Client, server, and UI integration were outside this review's scope.

Baseline command from this checkout:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run scripts/run-tests.luau
```

Observed: 14/14 modules passed, 195 individual tests ran. No production source was changed. The isolated executable reproduction is `scripts/review-repros.luau`; run it with:

```powershell
& 'C:/Users/asher/.cargo/bin/lune.exe' run scripts/review-repros.luau
```

Observed:

```text
fractional placement: false false 0 nil false false
fractional board child: true 0 64
NaN board child: true 0 64
perfectClearSetup one-move perfect clears: 0
```

## Finding: board-child validator accepts invalid coordinates (behavior defect)

**Current problem.** `UIControlRules.validateBoardGridChildren` (`src/shared/game/UIControlRules.luau`, around line 228) promises to validate 64 direct board cells with valid, unique coordinates. It checks numeric type and range but does not check that each coordinate is a finite integer. A 64-child grid with cell `(1,1)` changed to `(1.5,1)` returns `true`, zero problems, and 64 cells. Replacing that coordinate with NaN also returns `true`. Both are outside the logical `1..8` integer grid. The reproduction exercises the unchanged module and shows that `Grid.canPlace` rejects a fractional placement, so the two shared rules disagree about what a board coordinate is.

**Expected behavior and basis.** The checkout's `AGENTS.md` locks the board to exactly `Config.BoardSize × Config.BoardSize` (currently 8×8) and requires 64 direct cell buttons. `UIControlRules`' own contract says each child carries valid unique coordinates. A noninteger or nonfinite value cannot identify one of those 64 cells. The existing `boardGridRejectsBadCoordinates` test checks out-of-range and duplicate values, but does not check integral and finite values.

**Smallest proposed change.** In the shared validator, reject coordinates unless each is a finite integer within `1..Config.BoardSize`; add a characterization test to `UIControlRulesTests` for fractional and NaN attributes while retaining the valid 64-cell case. This is a validator-only change. Compatibility risk is low: a client that intentionally uses fractional board attributes would newly fail validation, but such attributes do not represent cells on this board. The current live client's attribute production was outside this headless review, so this finding establishes a broken guard, not an observed Studio rendering failure.

**Evidence path.** The locked board invariant and `Grid.canPlace`/`Grid.place` semantics led to the coordinate check. The existing tests showed range and duplicate coverage. The isolated reproduction then established the validator's true/zero-problem result for fractional and NaN coordinates.

## Discarded leads and limits

- `Grid.canPlace` initially looked vulnerable to fractional origins. The reproduction returned `false` and `Grid.place` left the board unchanged, so there is no board placement defect on that input.
- `DeveloperTools.boardPattern("perfectClearSetup")` cannot produce a one-placement perfect clear with any listed shape and legal origin; the exhaustive reproduction found zero such moves. The label does not establish that the pattern promises a one-placement clear, and a multi-placement setup may be intentional. No change is proposed without that product contract.
- `DeveloperTools.commandList` omits the accepted `ForceNoMove` action. No consumer of `commandList` exists within the reviewed shared modules; client/server integration is outside scope, so there is no proven user-visible defect here.
- No refactor met the requested threshold of a specific existing maintenance failure plus a concrete simpler design and characterization coverage. No style-only or metric-only suggestions are proposed.

Coverage gap: headless tests can prove these pure module behaviors, but they cannot establish whether Roblox Studio creates malformed board-cell attributes or how the live client reports this validator failure. No Studio play test was performed.

