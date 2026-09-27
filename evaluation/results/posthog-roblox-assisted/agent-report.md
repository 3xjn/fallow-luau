# Evaluation: stale feature flags after reload

## Symptom and affected source

`src/Server/FlagCache.luau` applied each successful `/flags` response over the existing store. When a later response omitted a formerly present flag, its value, payload, and v4 details remained. A flag retained in the response could also keep an old payload or details when the new entry omitted them. `FeatureFlags:reload` calls `FlagCache.applyResponse` for each successful fetch, while `docs/feature-flags.md` says a successful reload makes flags fresh and missing flags fall back to defaults.

## Reproduction and evidence

The new `tests/FlagCache.spec.luau` case applies a v4 response containing `old` and `retained`, then a second response containing only `retained` without metadata. Expected: `old` has no cached value, payload, or details; `retained` is false and has no prior payload or details. Observed before the fix: `old` still had a cached value, and the regression failed at `removed flag is no longer cached`.

The analyzer's `health --root . --complexity --file-scores --explain` report listed `FlagCache.applyResponse` among complex functions, which directed inspection to this path. Its score did not establish the defect. The source's merge behavior, the documented reload contract, and the failing executable regression did. The high complexity listings for `RelayServer:_handle` and other functions were navigation hints only; no behavior claim was inferred from them. There were no other investigated false leads.

## Change

`FlagCache.applyResponse` now clears values, payloads, and details after confirming the response is a table and before loading its v3 and v4 entries. Request metadata is still updated, v4 still overrides v3 within the same response, quota-limited responses still clear flags, and invalid non-table responses leave the store alone. This changes successful reload behavior for flags and metadata absent from the latest response: they now disappear as documented.

## Commands and results

Run from this checkout:

```text
& 'C:\Users\asher\Documents\ChatGPT\fallow-luau\target\evaluation\bin\fallow-luau.exe' health --root . --complexity --file-scores --explain
& 'C:\Users\asher\.cargo\bin\lune.exe' run tests/runTests.luau
```

The baseline suite passed: 48 passed, 0 failed; function coverage 39/39. With the regression added and production code unchanged, the suite failed: 48 passed, 1 failed, at the stale `old` value; coverage remained 39/39. After the repair, the full suite passed: 49 passed, 0 failed; function coverage 39/39.

## Uncertainties

The regression runs the pure cache module under Lune. The Roblox-bound reload path was inspected but not exercised in Studio. The behavioral claim is limited to successful decoded `/flags` responses passed through this cache.
