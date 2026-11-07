# XTermJS snapshots and bounded deltas

## Goal

- Fresh opening = latest snapshot + retained output after that snapshot.
- Keep the existing transport, acknowledgements, and reconnect behavior.
- Keep recovery data in server memory.
- Keep the existing tail capacity: 200,000 bytes in debug; 1,000,000 otherwise.
- Drop oldest output when that tail fills.

```text
PTY -> existing bounded TailStream -> existing throttled stream -> XTermJS
                   ^                                            |
                   |          snapshot upload every 5s          |
                   +--------------------------------------------+

Fresh opening: snapshot -> retained delta -> live output
Transport reconnect: existing Reopen behavior, same XTermJS
```

## Browser

- Use the serialization addon, pinned with XTermJS 6.0.0.
- Capture only after completed writes, at a safe parser boundary.
- Record the applied output position with the snapshot.
- Use one five-second timer and one asynchronous upload at a time.
- Skip unchanged terminals.
- Retry failed uploads on the next timer tick.
- Cancel the timer when the terminal is disposed.
- Restore the snapshot before applying streamed output.
- Keep serializer omissions in the small snapshot adapter.
- Include dimensions and mouse encoding in that adapter.

## PTY storage

- Extend `TailStream`; retain its existing reader and notification channels.
- Add output positions without changing byte-count flow control.
- Store one `OutputSnapshot { position, data }` beside the bounded tail.
- Limit the snapshot payload to the same capacity as the tail.
- Ignore older uploads. Reject future positions and oversized payloads.
- Select the snapshot and reader position under the tail's existing mutex.
- Keep trace logging, including output traces.
- Use a static atomic counter to identify processes within a server lifetime.
- Add no atomic stop flags, custom wakers, event journal, or acknowledgement ledger.

## API

ServerFn connects the browser to the web-facing server.
RemoteFn routes from that server to the PTY owner.
Both are required because the selected terminal may run on a remote server.

- Add `put_snapshot(terminal, snapshot)` and its unary RemoteFn.
- Upload `{ process, position, data }`.
- Validate that the process still matches before storing it.
- Extend Init with process ID, capacity, and an optional snapshot.
- Add a position to output messages.
- Preserve Create/Reopen, write, resize, pipe, and byte-count ack APIs.
- Send snapshot data once in Init; no separate snapshot-transfer protocol.
- Exclude snapshot bytes from PTY output acknowledgements.

## Eviction

- Tail eviction continues independently of snapshots and browser connectivity.
- If eviction overtakes the snapshot, restore it and replay the retained suffix.
- Log that intervening output was dropped.
- Missing mode changes or screen updates cannot be reconstructed.
- This recovery is best effort, as with the existing bounded tail.

## Validation

- PTY: snapshot followed by the exact suffix, without replaying its prefix.
- PTY: snapshot selection remains stable if another upload arrives during replay.
- PTY: eviction stays bounded while disconnected; snapshots survive eviction.
- PTY: reject oversized/future snapshots; ignore older uploads.
- Browser: restore a real mouse-capturing TUI after disconnected tail overflow.
- Browser: round-trip mouse protocols and encodings with the pinned addon.
- Browser: skip snapshots inside UTF-8 and CSI/OSC/DCS sequences.
- Browser: upload periodically, keep one upload in flight, and stop on disposal.
- Keep tests focused on snapshots and deltas.
- Remove the unrelated editor and UI changes from this PR.

Run Bazel first:

```sh
bazel test --test_output=errors //pty:pty-test //terminal:terminal-snapshot-integration-test-debug
bazel build //...
bazel test --test_output=errors --verbose_failures --flaky_test_attempts=3 //...
bazel test --test_output=errors --verbose_failures -c opt --flaky_test_attempts=3 $(bazel query 'attr("tags", "opt_mode", //...)')
bazel run //bazel:buildifier_check
```

Then mirror the Cargo checks from merge validation:

```sh
cargo build --bins --features=server,server-all,max_level_debug,debug,diagnostics
cargo build --bins --features=server,server-all,max_level_info --release
RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features
./demo/scripts/integration-test.sh target/debug/demo-server
./demo/scripts/integration-test.sh target/release/demo-server
```
