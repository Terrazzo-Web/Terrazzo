# Password Update Playwright Test

## Summary
Add a dedicated end-to-end Playwright test that proves Terrazzo can transition from “no password required” to “password required” without restarting the test harness manually. The test will start the existing terminal server through Bazel with a temp config file, update that config by calling the terminal CLI during the test, reload the browser, and verify that login is now required and succeeds with the new password.

## Key Changes
- [x] Add a new terminal CLI automation path:
  - Extend `terminal/src/backend/cli.rs` with `--password-stdin` for `--action set-password`.
  - Keep the current interactive prompt as the default; only switch to stdin when the flag is present.
  - Update `terminal/src/backend/config/password.rs` so password-setting logic can accept either interactive prompt input or a provided string from stdin, then persist the hashed password into the config file exactly as today.
- Add a Bazel/Playwright harness path for temp-config server startup:
  - Extend `bazel/playwright_test.sh` so a test target can opt into a temp config file in `TEST_TMPDIR`.
  - For this test mode, create the config file before server startup and seed it with:
    ```toml
    [server.config_file_poll_strategy]
    fixed = "1s"
    ```
  - Start the server with `--config-file <temp-config>` in addition to the existing dynamic port and endpoint-file args.
  - Export the resolved server binary path, `CARGO_MANIFEST_DIR`, the temp config path, and `TEST_TMPDIR` into the Playwright process so the spec can invoke the same binary reliably.
- Add a dedicated Playwright spec for password update:
  - Create a new spec alongside the existing terminal integration tests rather than folding this into the current general terminal spec.
  - In the spec, first verify the app auto-logs in when no password exists by waiting for the existing add-tab button selector.
  - Spawn the terminal binary from Node with `--config-file <temp-config> --action set-password --password-stdin`, write a random password to stdin, and wait for a zero exit status.
  - Reload the page and assert the auto-login path no longer succeeds.
  - Detect the password input from the login UI, enter the same password, and assert the add-tab button becomes visible again.
- Add/adjust Bazel test targets:
  - Add a new Playwright target pair for this spec, parallel to the existing debug/release terminal integration targets.
  - Use the existing `:terminal-server-debug` and `:terminal-server` binaries, but enable the temp-config mode only for this new password-update test target.

## Public Interfaces
- New CLI option on `terrazzo-terminal`:
  - `--password-stdin`
  - Valid only with `--action set-password`
  - Reads the password from stdin instead of prompting on the TTY
- New Playwright wrapper environment contract for opt-in tests:
  - Temp-config mode flag
  - Exported config-file path
  - Exported server binary path and manifest dir for child-process reuse

## Test Plan
- Run the new Playwright test in both:
  - `//terminal:terminal-password-update-test-debug`
  - `//terminal:terminal-password-update-test-release`
- Verify the happy path:
  - server starts from temp config with no password
  - initial page load reaches the terminal UI without login
  - CLI updates the config file successfully
  - server picks up the change via config polling
  - page reload eventually shows password login instead of immediate access; because the config file is polled once per second, the transition may take at least 1 second after the password update
  - correct password restores access
- Add one small Rust unit test around the non-interactive password input path if the implementation extracts a helper for “set password from provided string”; otherwise rely on the Playwright test as the primary coverage.

## Assumptions
- We will not automate the existing hidden prompt via PTY; the plan intentionally adds `--password-stdin` because it is simpler and more reliable under Bazel.
- The test only needs the positive flow after password creation; it does not need a wrong-password assertion unless requested later.
- A dedicated Playwright spec/target is preferable to changing the startup behavior of the existing terminal integration targets.
- No frontend production code changes are required beyond using the existing login input and add-tab selectors already present in the app.
