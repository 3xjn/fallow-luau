# Shared-rule repairs

Implemented the three behavior repairs accepted from `REVIEW.md`. The original `scripts/review-repros.luau` and review remain as pre-change evidence; that script now reaches the deliberate future-schema error at its final reproduction.

## Caller compatibility for profile reconciliation

Read both `ProfilePersistence.reconcileWithStored` call sites in `src/server/services/GameServer.server.luau` before changing its behavior:

- `updateProfileWithRetry` at line 915 calls it directly from `UpdateAsync` inside `pcall` (line 922). An error makes `ok` false; it follows its existing bounded retry and profile-save failure path. A new nil/sentinel return would have been treated as a successful `pcall`, so the helper now raises a deliberate error when the stored schema is newer.
- The receipt transform at line 1756 dereferences `payload.fulfilledPurchaseIds` immediately after reconciliation, also inside `pcall`. The new error exits before that dereference; the existing `not ok` path at line 1772 reports failure and returns `false`, leaving the receipt unfulfilled for retry. No server file was changed.

The guard checks the fresh `existingProfile` supplied to the transform, before allocating or mutating the payload. Same-version reconciliation still merges stored receipt and code ledgers while retaining session-owned fields. The new test checks both paths and that rejected future data and inputs remain untouched.

## UI rules

`validateBoardGridChildren` now requires finite integer `boardX` and `boardY` values in the existing range. Tests retain the valid 8x8 case and reject fractional, NaN, and infinite values on both axes.

`consumeActionCooldown` now accepts and records the first request for an unseen key. Later requests keep the existing cooldown decision; a rejected repeat does not update the timestamp. The new test covers first, repeated, and post-cooldown requests. NaN time remains rejected.

## Evidence

From the assigned checkout:

```powershell
& 'C:\Users\asher\.cargo\bin\lune.exe' run scripts/run-tests.luau
& '.\tools\stylua.exe' --check src/shared/game/ProfilePersistence.luau src/shared/game/ProfilePersistenceTests.luau src/shared/game/UIControlRules.luau src/shared/game/UIControlRulesTests.luau
git diff --check
```

Before production edits, the expanded suite failed in `ProfilePersistenceTests` (`UpdateAsync must abort rather than overwrite a future profile`) and `UIControlRulesTests` (`A key without a prior request should be accepted immediately`): 12/14 modules passed, 159 tests completed before those modules stopped. The original reproduction had already observed acceptance of fractional and NaN board coordinates. After the repairs, the full suite passed 14/14 modules and 198 tests, up from the original 195. The remaining headless limit is the actual Roblox `UpdateAsync` and Studio integration path; the report makes no live DataStore or Studio Play claim.
