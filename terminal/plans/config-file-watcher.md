# Replace periodic config file polling with a file watcher

## Summary
Replace the current config reload loop, which only polls the config file on a timer, with a combined reload pipeline that supports:

- a file-watcher strategy enabled by default
- the existing `config_file_poll_strategy` setting as an optional polling fallback
- running both mechanisms at the same time

The end result should preserve today’s behavior for deployments that rely on polling, improve reload latency for local edits and CLI-driven config writes, and add integration coverage for the watcher-driven path.

## Status Quo
- `Config::into_dyn` in `terminal/src/backend/config/into_dyn.rs` always spawns `poll_config_file(...)` when `--config-file` is present.
- `poll_config_file(...)` sleeps according to `server.config_file_poll_strategy`, compares the file’s modification timestamp, and reloads `ServerConfig` plus `letsencrypt` when the timestamp changes.
- `config_file_poll_strategy` is currently the only reload mechanism exposed in config.
- The repo already has a `notify`-based watcher implementation under `terminal/src/text_editor/notify/`, so there is prior art for watcher lifecycle, debouncing concerns, and error handling.
- The existing integration test `terminal/tests/integration-test-password-update.spec.mjs` proves that config polling eventually applies a password change written by the CLI.

## Desired Behavior
- File watching is the default behavior for config-file reloads.
- `config_file_poll_strategy` continues to work for users who want periodic polling.
- Users can enable both watcher-driven reloads and polling together.
- A config rewrite caused by the CLI should usually reload promptly via the watcher, without needing a short poll interval.
- If the watcher misses an event or the platform delivers less reliable notifications, polling can still catch the update when enabled.
- Reloading should keep the existing “best effort” posture: log failures, keep the server running, and continue listening for future changes.

## Key Changes
- [ ] Extend server config to represent config-file watching explicitly:
  - Add a new server-level setting alongside `config_file_poll_strategy`, for example `config_file_watcher: bool` or another small dedicated watcher strategy type.
  - Default the watcher setting to enabled when the config file omits it.
  - Preserve `config_file_poll_strategy` as optional config instead of treating it as the always-on default mechanism.
  - Ensure `Config::to_config_file()` and `ConfigFile::merge(...)` round-trip the new setting cleanly.
- [ ] Refactor config reload startup around a shared “reload coordinator”:
  - Replace the unconditional `tokio::spawn(poll_config_file(...))` in `Config::into_dyn`.
  - Introduce a small helper that inspects the runtime config and starts zero, one, or two background tasks:
    - watcher only
    - polling only
    - watcher plus polling
  - Keep the task startup gated on `cli.config_file` being present, just like today.
- [ ] Extract the actual reload work into one shared function:
  - Move the “load file, merge with CLI defaults, update dynamic config, refresh runtime strategies” logic out of `poll_config_file(...)` into a helper such as `reload_config_file(...)`.
  - Have both the watcher path and the polling path call that same helper so they cannot drift.
  - Preserve the existing selective application behavior in `apply_server_config(...)` and `apply_letsencrypt_config(...)`.
  - Consider whether mesh config should also be refreshed while touching this code; if the current omission is intentional, document that in comments or the plan implementation notes.
- [ ] Add a watcher-backed reload task:
  - Reuse the existing `notify` crate already present in the repo instead of introducing a second watcher stack.
  - Create backend-focused watcher glue in `terminal/src/backend/config/` rather than coupling config reloads to the text editor’s UI-side watcher service.
  - Watch the config file path directly when possible, or its parent directory if the platform requires that for atomic-save workflows.
  - Treat create/modify/rename events that affect the config path as reload triggers.
  - Expect editors and the CLI to sometimes rewrite the file atomically; the watcher logic should therefore handle replacement of the watched inode/path, not only in-place writes.
- [ ] Keep polling semantics available without making them mandatory:
  - Change `config_file_poll_strategy` handling so it is optional at runtime.
  - If the poll strategy is unset, skip starting the polling task instead of silently falling back to a 60-second poll loop.
  - If the poll strategy is set, polling continues exactly as a periodic fallback mechanism.
  - If both watcher and polling are enabled, allow duplicate reload attempts but make them harmless through idempotent reload application.
- [ ] Harden duplicate-trigger behavior:
  - Because one user action may emit multiple watcher events and may also be picked up by polling, guard against noisy logs or pointless config churn.
  - Keep a lightweight “last applied fingerprint” in the reload path, such as modified timestamp or file contents hash, to short-circuit exact repeats.
  - Preserve the current behavior where a failed parse/read does not overwrite the active config.
- [ ] Update logging and observability:
  - Distinguish watcher-triggered reloads from polling-triggered reloads in tracing spans and log lines.
  - Log watcher startup, polling startup, and watcher creation failures explicitly.
  - If watcher startup fails and polling is also disabled, warn clearly that live reload is unavailable for the config file.

## Implementation Notes
- `terminal/src/backend/config/server.rs`
  - Add the new watcher setting to `ServerConfig<T>`.
  - Update comments so the config surface describes watcher vs polling clearly.
- `terminal/src/backend/config/merge.rs`
  - Parse and default the new watcher setting.
  - Make `config_file_poll_strategy` optional at runtime if that yields the cleanest “polling disabled” representation.
  - Serialize both settings back out in `Config::to_config_file()`.
- `terminal/src/backend/config/into_dyn.rs`
  - Replace `poll_config_file(...)` as the single entry point with a coordinator plus shared reload helper.
  - Keep `apply_server_config(...)` and `apply_letsencrypt_config(...)` as the final mutation boundary unless refactoring reveals a better seam.
- `terminal/src/backend/config/`
  - Add a small watcher-specific helper module if needed, rather than overloading `into_dyn.rs` with all watcher setup details.
  - Keep the code backend-local even if pieces of `terminal/src/text_editor/notify/watcher.rs` inspire the implementation.

## Public Interfaces
- New config-file setting in `[server]`:
  - `config_file_watcher = true` by default, or equivalent schema if a different name/type reads better in the codebase
- Existing config-file setting continues to work:
  - `[server.config_file_poll_strategy]`
  - may now be omitted entirely without losing live reload, because watcher mode is the default
- Supported combinations:
  - watcher only
  - polling only
  - watcher plus polling

## Integration Test Plan
- [ ] Extend the Playwright/Bazel temp-config startup used by the password update flow so it can seed watcher settings in addition to poll settings.
- [ ] Reuse `terminal/tests/integration-test-password-update.spec.mjs` as the base scenario:
  - start the server with a temp config that enables the file watcher
  - set `[server.config_file_poll_strategy]` to a very long interval such as `fixed = "1h"` so polling cannot realistically satisfy the test quickly
  - update the config by invoking the CLI `set-password` action exactly as the current test does
  - reload the page and assert that password login appears promptly, proving the watcher path applied the change
- [ ] Update the assertion text in the spec so it no longer claims the updated password arrived “after the config poll”; it should mention live config reload more generically or explicitly mention the watcher path.
- [ ] Keep the existing integration coverage for polling semantics in one of these forms:
  - preserve the current spec/target with `fixed = "1s"` as a polling-focused test, or
  - parameterize the spec so one variant exercises watcher mode and another exercises polling mode
- [ ] Add at least one integration case for the combined mode:
  - start with watcher enabled and polling configured to `1h`
  - verify the same password update succeeds, primarily to prove that enabling both does not interfere with watcher behavior
  - this can be the main watcher test if a separate watcher-only mode is not worth the extra target count
- [ ] Keep the test runtime stable:
  - avoid waiting for the hour-long poll interval
  - rely on watcher-driven propagation and a bounded reload loop in the browser similar to the current helper
  - if the watcher path proves slightly asynchronous on CI, use a short retry window rather than fixed sleeps

## Rust Test Plan
- [ ] Add unit tests around config merging/serialization:
  - watcher defaults to enabled when absent from file config
  - polling can be disabled by omission
  - both settings round-trip through `to_config_file()` and `merge(...)`
- [ ] Add focused tests for reload deduplication or trigger handling if the implementation introduces a helper for event filtering/fingerprinting.
- [ ] If watcher setup code is isolated cleanly, add a small backend test for “relevant file event triggers reload” vs unrelated events being ignored.

## Validation
- Run formatting:
  - `cargo +nightly fmt`
- Run targeted integration coverage first:
  - the existing password-update Playwright target(s)
  - the new or updated watcher-oriented Playwright target(s)
- Then run broader validation:
  - `bazel query 'attr("tags", "auto-generated", //...)' | xargs -r -n1 bazel run`
  - `bazel run //bazel:buildifier`
  - `bazel test //...`

## Risks And Mitigations
- Atomic-save behavior may replace the file rather than modify it in place:
  - mitigate by watching rename/create events and, if needed, the parent directory
- Watchers can emit duplicate events:
  - mitigate with a lightweight fingerprint or “no-op if unchanged” check before applying config
- Watchers are less reliable on some environments:
  - mitigate by allowing polling to remain configured as a fallback
- Startup failures in watcher creation could silently disable reloads:
  - mitigate with explicit warnings and by preserving polling when configured

## Assumptions
- The new watcher setting belongs in server config rather than CLI flags, because the TODO asks for the watcher to be part of default config behavior.
- The project wants backward compatibility for existing config files that already specify `config_file_poll_strategy`.
- Reusing the existing password-update integration flow is preferable to inventing a new end-to-end scenario, because it already proves an externally visible config change.
- It is acceptable for both watcher and polling tasks to call the same reload helper concurrently as long as applying the same config twice is effectively a no-op.
