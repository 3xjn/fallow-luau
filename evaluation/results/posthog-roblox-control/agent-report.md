# Local evaluation

## Symptom and evidence

`ReloadFeatureFlags` is documented to make flags fresh after a successful fetch (`docs/feature-flags.md`, "Reloading"). `src/Server/FeatureFlags.luau` calls `FlagCache.applyResponse` for each successful response, but `src/Server/FlagCache.luau` previously merged new entries into the existing store. A flag omitted from a later response therefore retained its old enabled value, payload, and metadata. This can leave a removed or no-longer-targeted flag enabled for a player.

Affected source: `src/Server/FlagCache.luau`.

## Reproduction and repair

The new test in `tests/FlagCache.spec.luau` applies a response containing enabled `old` with a payload and metadata, then applies a successful response containing only `new`. Expected: `old` has no value, payload, or details and `new` is enabled. Observed before the repair: `old` was still enabled, and the test failed at `removed flag must not remain enabled`.

`FlagCache.applyResponse` now clears the three flag maps before applying a table response. It preserves the store table and the public API. The behavior change is that each successful fetch replaces prior flag values, payloads, and details; a flag absent from the response no longer remains cached. A non-table response still leaves the store unchanged. The existing quota-limited response behavior remains an empty flag store with request metadata.

## Commands and results

All commands ran from this checkout:

```text
C:/Users/asher/.cargo/bin/lune.exe run tests/runTests.luau
```

- Baseline: 48 passed, 0 failed; function coverage 39/39.
- With the new regression and original source: 48 passed, 1 failed. The failing assertion was `removed flag must not remain enabled`.
- After the repair: 49 passed, 0 failed; function coverage 39/39.

```text
git diff --check
```

Passed after the repair.

## False leads and uncertainties

I inspected the queue's handling of HTTP 413 and concurrent enqueues as possible loss scenarios, but did not establish a focused safe repair for either. The flag reload defect was directly supported by the documented fresh-reload behavior and the failing regression.

The regression exercises the pure cache under Lune. It does not run a live Roblox server or a PostHog `/flags` request, so integration behavior remains unverified here.
