# Accepted repairs

Implemented review findings 1-3 in `src/init.luau`. Production source and tests in this assigned checkout only; `REVIEW.md` and both review reproduction scripts remain intact.

## Changes

- Renamed the `pcall` error result in `notifyEffect`, `effect`, and `effectScope`, so each branch raises the original callback error instead of trying to call its error string.
- Stored each tracking pause as a non-nil frame, allowing nested pauses to restore a previous nil state and then the original subscriber in order.
- On failed effect creation, detached the new effect's dependencies before rethrowing. Parent/scope ownership links are now added only after successful initial execution, so a failed child is not retained by its parent. On failed scope creation, detached its child effects before rethrowing.

## Fail-before / pass-after evidence

Commands below were run from this checkout with `C:/Users/asher/.cargo/bin/lune.exe`.

| Command | Before repair | After repair |
| --- | --- | --- |
| `lune run tests/effect.spec` | Exit 1, `src:52: attempt to call a string value` while a failed effect was retained. | Included in passing full suite; tests now assert failed root and nested effects are detached and original initial/rerun error messages survive. |
| `lune run tests/effectScope.spec` | Exit 1 at `shouldDisposeScopeChildrenWhenCreationFails`. | Included in passing full suite; the failed scope's child no longer runs on a later signal update. |
| `lune run tests/untrack.spec` | Exit 1 at `shouldRestoreTrackingAfterNestedPauses`. | Included in passing full suite; the effect runs twice after a matched nested pause/resume sequence and signal update. |
| `lune run tests` | Before adding regressions: `All tests passed!` | After repair: `All tests passed!` (exit 0). |

The preserved `tests/review-repros.luau` now prints matching actual/expected run counts (`2/2` for nested pause, `1/1` for failed effect, `1/1` for failed scope) and includes the original sentinel error text in both initial and rerun cases. `git diff --check` exited 0; Git printed only CRLF conversion warnings.

## Risks and remaining policy choice

Successful nested effects are linked to a parent after their initial callback returns, instead of before it runs. Existing effect-order and topology tests pass, including the new nested failure assertion, but code that inspects intermediate graph state during a child's initial callback may observe different timing. Raised errors now expose the application failure instead of the accidental secondary message. Failed creation no longer leaves callbacks active without a stop handle.

Review finding 4 was not changed. `lune run tests/review-notification-error` still shows a remaining queued notification delivered during a later unrelated update after another effect throws. The local contract does not choose whether to drain or discard remaining notifications on failure, so this implementation preserves that scheduling behavior pending a separate policy decision.
