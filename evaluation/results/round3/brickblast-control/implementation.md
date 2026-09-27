# Board coordinate validator repair

`UIControlRules.validateBoardGridChildren` now rejects fractional and NaN cell coordinates. The existing numeric type and `1..Config.BoardSize` bounds checks remain in place; the added `x % 1 ~= 0` and `y % 1 ~= 0` checks require integer positions. `UIControlRulesTests` now covers fractional X, fractional Y, and NaN alongside the existing valid 64-cell grid and other invalid-coordinate cases. No other production behavior was changed.

## Executable evidence

All commands were run from this assigned checkout.

1. Added the regression test before changing production source, then ran:

   ```powershell
   & 'C:/Users/asher/.cargo/bin/lune.exe' run scripts/run-tests.luau
   ```

   Observed exit 1: `UIControlRulesTests` failed at the new fractional-X assertion, `A fractional X coordinate cannot identify a board cell`; the remaining 13 modules passed. This is the fail-before result.

2. Added the coordinate checks, then reran the same command. Observed exit 0: **14/14 modules passed, 196 individual tests ran** (the baseline was 195).

3. Ran the preserved isolated reproduction:

   ```powershell
   & 'C:/Users/asher/.cargo/bin/lune.exe' run scripts/review-repros.luau
   ```

   Observed `fractional board child: false 1 64` and `NaN board child: false 1 64`. The unrelated diagnostic lead remains as recorded in `REVIEW.md` and was not implemented.

4. Ran `git diff --check`; exit 0, no output.

`REVIEW.md` and `scripts/review-repros.luau` were preserved. Verification is limited to the headless shared-module test runner; no Roblox Studio client/server integration test was performed.
