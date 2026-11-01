# XTermJS state snapshots and bounded replay

## Goal

- Preserve TUI state when attaching a fresh XTermJS.
- Restore mouse handling, screen contents, and supported terminal modes.
- Store a browser-generated snapshot plus a bounded output tail.
- Drop oldest output when the tail is full.
- Keep detached processes running.
- Store recovery state in server memory.
- Add no disk persistence or server-side emulator.

## Primer: ServerFn and RemoteFn

There are two boundaries:

```text
Browser                 Web-facing server           PTY-owning server
   |                           |                           |
   +-- ServerFn over HTTP ----->|                           |
                               +-- RemoteFn via mesh ----->|
                               |                           +-- process operation
                               |<------- result -----------+
   |<--------- result ---------+

For a local terminal, both server roles are the same process.
RemoteFn dispatch then invokes the local handler.
```

ServerFn: browser to server

- Declared with `#[server(...)]` in `terminal/src/terminal/api.rs`.
- Generates the browser-side request function and server-side handler.
- Encodes arguments and results for HTTP transport.
- Uses JSON for ordinary terminal operations.
- Uses streaming text for the multiplexed output pipe.
- Keeps PTY access in server-only code.
- Runs through the application's existing authentication and routing setup.
- We need it because browser code cannot access server memory or PTYs directly.

RemoteFn: server to the process owner

- Declared with `declare_remote_fn!` in terminal service modules.
- Registers a named operation with typed inputs and outputs.
- `.call(address, args)` selects the target through `ClientAddress`.
- A local target invokes the registered handler locally.
- A remote target uses the existing mesh routing and RPC transport.
- Unary RemoteFns return one result.
- Streaming RemoteFns return a stream of results.
- We need it because the browser's server may not own the selected terminal.
- The process map and recovery state live on the owning server.

Existing write example:

```text
Browser: api::write(terminal, data)
  -> ServerFn HTTP handler
  -> service::write
  -> WRITE_FN.call(terminal.via, ...)
  -> owning server: processes::write
  -> PTY input
```

How this applies to snapshots:

- Add a snapshot-upload ServerFn for browser access.
- Add a unary RemoteFn to reach the PTY owner.
- Store the snapshot beside that owner's process entry.
- Change attach requests at both boundaries.
- Carry restore events through the existing streaming RemoteFn.
- Deliver them through the existing browser output pipe.
- Forward acknowledgement and resize changes through both layers too.
- Do not add a separate transport or put snapshots in gateway memory.

Naming in this plan:

- `put_checkpoint` is the browser-facing upload operation.
- Its RemoteFn forwards the request to the PTY owner.
- `commit_checkpoint` is the internal storage operation.
- `ack_applied` replaces the existing `ack` semantics.
- These internal names do not imply extra HTTP endpoints.

## Current behavior

- `attach.rs` creates a fresh `TerminalJsRc`.
- `javascript.js` creates XTermJS with default modes.
- Fresh attachment requests `RegisterTerminalMode::Create`.
- For an existing process, the server rewinds its output lease.
- `pty/src/tail.rs` replays only retained byte chunks.
- Older initialization sequences may already be gone.
- The screen can look correct while mouse reporting is disabled.
- Transport reconnect uses `Reopen` and keeps the existing XTermJS.
- It currently relies on the server lease cursor.

Current buffer limits:

- With the `debug` feature: **200,000 bytes** (about 195 KiB).
- Otherwise: **1,000,000 bytes** (about 977 KiB).
- Source: `terminal/src/api/shared/terminal_schema.rs`.
- The same constant currently controls transport credit and replay capacity.
- Give these settings separate names. Keep the existing replay sizes.

## Design overview

```text
Browser                         PTY-owning server
                                ProcessIoEntry
 XTermJS <--- ordered events -----+-- bounded output journal <--- PTY reader
   |                            |          |
   +--- snapshot upload --------+          +-- evict oldest when full
                                |
                                +-- latest accepted snapshot

Intact history:
 [snapshot N] [events N+1 ... H] [live events ...]

After overflow:
 [snapshot N] [missing events] [retained tail ...] [live ...]
              ^ explicit Gap; recovery is best effort
```

Rules:

- Replay each retained event once.
- Report missing intervals explicitly.
- Never block PTY reads to retain history.
- Fence stale requests with attachment tokens.
- Distinguish process instances with an incarnation ID.
- Separate transport acknowledgements from snapshot acceptance.
- Serialize snapshot commits, journal updates, and attachment handoffs.

## Serializer compatibility

- Add `@xterm/addon-serialize` to `terminal/assets/jsdeps/`.
- Pin a version compatible with the repo's XTermJS 6.x.
- Export it through `JsDeps`.
- Include terminal modes and the alternate screen.
- Include up to 1,000 normal-buffer scrollback rows.
- Reduce scrollback to fit the snapshot byte limit.

The addon is not a complete internal checkpoint:

- Audit its coverage before implementing the protocol.
- Supplement missing state with a tested adapter.
- Current upstream restores mouse tracking but omits mouse encoding.
- SGR mouse encoding (`?1006`) is essential for this bug.
- Use parser hooks where possible.
- Isolate any private XTermJS API behind a version-pinned adapter.

Round-trip fixtures must cover:

- Mouse tracking and supported mouse encodings.
- Actual wheel reports sent to the PTY.
- Alternate-screen entry and exit.
- Saved cursor and scroll margins.
- Cursor visibility and text attributes.
- Application cursor keys and keypad modes.
- Bracketed paste, origin mode, and wrapping.
- Tab stops and character sets.
- Wide characters and wrapped lines.
- Split UTF-8, CSI, OSC, and DCS input.

Snapshot boundaries:

- Capture only after XTermJS finishes applying an output event.
- Require a safe parser and UTF-8 boundary too.
- A completed write callback alone is insufficient.
- Defer capture while a sequence is incomplete.
- Test boundary detection in the adapter.
- Do not claim exact recovery for unsupported state.

References:

- [Serializer API](https://github.com/xtermjs/xterm.js/blob/master/addons/addon-serialize/typings/addon-serialize.d.ts)
- [Serializer implementation](https://github.com/xtermjs/xterm.js/blob/master/addons/addon-serialize/src/SerializeAddon.ts)

## PTY API changes

Extend `TailStream` into a bounded replay journal:

- Preserve oldest-first eviction.
- Add event IDs and absolute output byte offsets.
- Add ordered resize events.
- Keep snapshot payloads opaque to the PTY crate.
- Replace silent cursor advancement with explicit Gap events.

Proposed in-memory structure:

```text
ProcessIoEntry
  incarnation
  active_attachment
  input: existing PTY writer
  journal
    checkpoint: Option<CheckpointEnvelope>
    base_event / next_event
    output_end
    events: VecDeque<JournalEvent>
    retained_bytes / capacity
    size_at_retained_start

JournalEvent
  Output { event_id, start_byte, bytes }
  Resize { event_id, size }

CheckpointEnvelope
  event_id / output_end
  size / format_version
  payload
```

Cursor rules:

- Event IDs define replay order.
- Byte offsets define output accounting.
- Resizes advance event IDs without advancing byte offsets.
- Scope both cursors to the process incarnation.
- Encode 64-bit wire cursors without JavaScript precision loss.
- Decimal strings are sufficient.

Proposed operations:

- `attach(Fresh | Resume { incarnation, applied_event })`
  - Revoke the old lease.
  - Issue an attachment token.
  - Return restore metadata and retained events.
  - Emit Gap when required history was evicted.
- `commit_checkpoint(token, envelope)`
  - Validate ownership, format, size, and cursor.
  - Require an applied prefix acknowledged by this attachment.
  - Install the checkpoint and compact covered history atomically.
  - Do not assume its suffix survived capture and upload.
- `ack_applied(token, event_id, output_end)`
  - Accept only monotonic acknowledgements.
  - Reject positions beyond the sent prefix.
- `resize(token, size)`
  - Apply the ioctl and record an ordered resize event.
  - Supply initial dimensions when opening a process.

Lease migration:

- Replace `lease_output(rewind: bool)` in the terminal path.
- Preserve cancellation-safe lease handoff.
- Never replace a process after a canceled handoff.
- Extend `LeaseItem` with cursor and restore metadata.
- Check other PTY consumers before removing old APIs.
- Keep compatibility wrappers where needed.

## Server memory and eviction

Ownership:

- Store recovery state in the PTY-owning `ProcessIoEntry`.
- Share it through the existing process map.
- Forward remote operations to that server.
- Do not store authoritative state in gateway pipes.
- Close/EOS cleanup releases snapshots and journal data.
- Server restart loses recovery state with the PTY session.

Output journal:

- Keep the current 200,000 / 1,000,000-byte capacity.
- Evict oldest chunks until new output fits.
- Count bytes, not rendered rows.
- Retain only a bounded suffix of oversized appends.
- Bound event metadata too.
- Coalesce resize-only history as needed.
- Preserve dimensions at the retained tail's start.
- Continue reading the PTY while disconnected.

Snapshots:

- Keep one accepted snapshot per process.
- Cap its payload at the process's output-buffer capacity.
- This storage is additional to the output journal.
- Reduce included scrollback if the snapshot is too large.
- Never truncate serialized VT output.
- Reject snapshots whose required visible state cannot fit.
- Keep the previous snapshot after rejection or upload failure.
- Continue normal journal eviction independently.
- Bound upload, transfer, and replacement buffers.
- Measure temporary replacement overhead.

Slow readers:

- Share immutable payloads.
- Do not pin an entire historical suffix.
- Hold cursors and bounded in-flight chunks.
- Emit Gap if eviction overtakes a reader.
- Apply this during restore and live streaming.
- Network credit may pause delivery.
- It must not pause the PTY reader for history retention.

## Recovery after eviction

- Send `Gap` before replaying a discontinuous tail.
- Include the missing range and retained-start dimensions.
- Restore the latest snapshot when available.
- Cancel partial parser and UTF-8 input before replaying the tail.
- Use a tested resynchronization strategy.
- Preserve restored modes during parser cancellation.
- Apply the retained-start dimensions.
- Show a short notice that output was dropped.

Accepted limits:

- Recovery across dropped output is best effort.
- A snapshot preserves modes established before capture.
- Dropped mode changes cannot be reconstructed.
- Dropped screen transitions can leave stale screen state.
- Arbitrary byte suffixes may start inside control sequences.
- A later snapshot records the resulting browser state.
- It cannot recover missing state.

## Terminal and remote API changes

Replace `RegisterTerminalMode::{Create, Reopen}`:

- Add an explicit Fresh/Resume strategy.
- Separate process creation permission from restore strategy.
- Carry the request through stream registration and `add_stream`.
- Update the remote `terminal.stream` function too.

```text
AttachRequest
  protocol_version
  terminal_def
  create_if_missing
  restore:
    Fresh
    Resume { incarnation, applied_event, output_end }

Server stream
  Init { incarnation, attachment, strategy, base_event, size, format_version }
  SnapshotStart { total_bytes, checkpoint metadata }
  SnapshotChunk { index, bytes }
  SnapshotEnd
  Gap { from_event, to_event, retained_start, size_at_retained_start }
  Output { event_id, start_byte, bytes: Utf8 | Base64 }
  Resize { event_id, size }
  Ready { replay_through }
  ... live events ...
  Eos | Revoked | Error { code, details }
```

Stream rules:

- Keep exact output bytes with the current UTF-8/base64 encoding.
- Snapshot bytes do not advance PTY output offsets.
- Give snapshot transfer separate bounded credit.
- Capture the replay high-water point during lease creation.
- `Ready` marks completion through that point.
- Report later eviction even during snapshot transfer.

Snapshot upload:

- Add `put_checkpoint(terminal, incarnation, attachment, checkpoint)`.
- Add its authenticated server function and remote unary function.
- Follow the existing resize/ack routing pattern.
- Return `Accepted`, `AlreadyAccepted`, or a structured rejection.
- Support retries after lost responses.
- Reject stale attachments, invalid cursors, and oversized payloads.
- Validate metadata; the server cannot verify snapshot semantics.

Acknowledgements and ownership:

- Replace byte-count acknowledgements with cumulative applied cursors.
- Include the incarnation and attachment token.
- Derive outstanding transport bytes from validated positions.
- Fence write and resize requests with the same token.
- Include attachment identity in pipe dispatch.
- Ignore late chunks from revoked streams.
- Make throttle cleanup generation-specific.
- An old stream must not remove its replacement's throttle entry.

Authorization and compatibility:

- Reuse session authorization and remote ownership checks.
- Enforce upload limits before large allocations where possible.
- Keep existing terminal list and title metadata.
- Log sizes and cursors, never terminal contents.
- Require matching protocol versions.
- Reject incompatible clients and remote servers explicitly.

## Resize ordering

- Serialize journal appends, resize ioctls, and snapshot commits.
- Record each successful resize before the next output append.
- Use that order as the browser's authoritative event order.
- Do not infer causal timing for bytes already buffered by the OS.
- Measure desired browser dimensions without immediately resizing XTermJS.
- Submit a resize request.
- Apply the resulting Resize event in the output queue.
- Serialize concurrent resize requests.
- Remove artificial shrink/sleep/grow resizing on attachment.
- Supply initial PTY dimensions before spawning where possible.
- Test resize latency and redraw behavior.

## Browser capture and scheduling

Application queue:

- Use one queue per attachment.
- Order output, resize, restore, and snapshot capture together.
- Advance the applied cursor after XTermJS's write callback.
- Capture snapshot, cursor, and dimensions in one synchronous step.
- Require a safe parser boundary.
- Upload asynchronously while output continues.
- Allow one upload at a time.
- Ignore stale completion callbacks.

Triggers:

- Trigger after 1,000 processed LF characters or 256 KiB of output.
- LF count measures activity, not rendered rows.
- The byte trigger covers TUI redraws without line feeds.
- Throttle captures to once per five seconds.
- Keep a pending trigger during cooldown or upload.
- Do not restart the timer on every byte.
- Continuous output must not postpone capture forever.
- Capture an initial snapshot after restore/replay.
- Respect the throttle for initial capture too.
- Retry failed uploads with bounded backoff.
- Do not depend on unload-time uploads.

Restore:

- Create or reset XTermJS for Fresh attachment.
- Set the snapshot's original dimensions.
- Restore VT output and supplemental state in the tested order.
- Replay retained events and process any Gap.
- Suppress synthetic restore replies to the PTY.
- Enable input after Ready.
- Request the measured browser size afterward.
- Avoid rendering incomplete restore frames where practical.

## Reconnect flows

```text
Fresh XTermJS / page reload        Existing XTermJS / transport reconnect
-------------------------       -----------------------------------
Fresh attach                    Resume(last applied cursor)
  -> Init + optional snapshot     -> new attachment token
  -> restore original size        -> unapplied retained events
  -> retained events + any Gap    -> any Gap
  -> Ready                        -> Ready
  -> input + requested resize     -> input
  -> initial/scheduled snapshot
```

- Without a snapshot, replay from process start if retained.
- Otherwise emit Gap and replay the available tail.
- Resume keeps the existing XTermJS state.
- Never replay a snapshot onto an already populated Resume terminal.
- If Resume history was evicted, emit Gap and continue best effort.
- If the incarnation changed, return `FreshRestoreRequired`.
- If the process is gone, return `ProcessGone`.
- Do not silently start a shell under an existing terminal display.
- Settle queued writes before selecting the Resume cursor.
- Generation-check asynchronous callbacks after replacement.
- Revoked ownership must not trigger automatic reacquisition.

## Implementation sequence

1. Add serializer round-trip fixtures and the state adapter.
2. Add bounded journal events, cursors, Gap, and checkpoint storage in `pty/`.
3. Update terminal schema, remote functions, ownership, and acknowledgements.
4. Add ordered resize handling and generation-safe stream cleanup.
5. Add browser restore, capture scheduling, and uploads.
6. Remove terminal rewind fallback and forced resize redraw.
7. Document memory limits and overflow behavior.

Primary files:

- `pty/src/{lib,tail,lease}.rs` and a replay journal module.
- `terminal/src/processes/{mod,stream,resize}.rs`.
- `terminal/src/api/shared/terminal_schema.rs`.
- `terminal/src/terminal/{api,client,attach,javascript}.rs`.
- `terminal/src/terminal/javascript.js`.
- `terminal/src/terminal/service/` and `streams/`.
- `terminal/src/backend/throttling_stream.rs`.
- JS dependency sources, lockfile, and generated bundle.

## Validation

The names below are proposed tests to implement, not existing coverage.

### PTY journal tests

Location: a new `pty/src/replay/tests.rs`, or `tail.rs` tests if extended in place.
Use synthetic output and tiny capacities. Avoid timing-dependent sleeps.

- `fresh_attach_replays_checkpoint_and_exact_suffix`
  - Append events 1 through 6. Commit a checkpoint at 3.
  - Attach Fresh. Expect checkpoint 3, then events 4, 5, 6.
  - Append event 7 during replay. Expect it once after the replay boundary.
- `fresh_attach_without_checkpoint_replays_retained_start`
  - Attach before any checkpoint exists.
  - With intact history, expect all events and initial dimensions.
  - After eviction, expect Gap, retained-start dimensions, and the tail.
- `capacity_overflow_evicts_oldest_without_stopping_reader`
  - Feed several times the configured capacity without an attached reader.
  - Expect the newest suffix and bounded retained bytes.
  - Assert the producer completes without a reader acknowledgement.
- `oversized_append_retains_bounded_suffix`
  - Append a chunk larger than the entire capacity.
  - Expect no panic, a bounded suffix, and a reported missing range.
- `resize_only_history_has_bounded_metadata`
  - Submit many resizes without output.
  - Expect bounded event storage and correct retained-start dimensions.
- `eviction_overtakes_reader_emits_gap`
  - Pause a reader, then overflow the journal.
  - Resume it. Expect Gap before the available tail.
  - Repeat eviction while replaying. Expect another Gap, never silent skipping.
- `checkpoint_commit_does_not_require_retained_suffix`
  - Capture at N. Evict N+1 before accepting the upload.
  - Accept the valid checkpoint. Fresh attach must still report Gap.
- `checkpoint_commit_and_append_preserve_event_order`
  - Control append, commit, and attach with barriers.
  - Exercise both orderings at each boundary.
  - Assert one consistent checkpoint/suffix pair and no duplicate events.
- `checkpoint_validation_rejects_invalid_metadata`
  - Table-test stale tokens, wrong incarnation, and unsupported formats.
  - Include future cursors, unapplied cursors, and size mismatches.
  - Expect structured rejection and no stored-state change.
- `checkpoint_retry_is_idempotent`
  - Accept an upload, lose its response, and retry it.
  - Expect AlreadyAccepted and unchanged storage.
- `snapshot_size_limit_preserves_previous_checkpoint`
  - Install a valid checkpoint, then submit an oversized replacement.
  - Expect rejection. Keep the previous checkpoint and normal eviction behavior.

Location: `pty/src/lease.rs` tests.

- Keep `canceled_handoff_can_be_resumed`.
- Keep `canceled_handoff_preserves_output_returned_by_drop`.
- Keep `concurrent_handoffs_are_serialized`.
- Adapt each test to the new attach request and cursor types.
- Add `revoked_attachment_cannot_mutate_process`.
  - Replace attachment A with B.
  - Reject A's writes, resizes, acknowledgements, and checkpoint commits.
  - Verify B still works.

### Server and protocol tests

Locations: `terminal/src/processes/stream/tests.rs`, service tests, and throttle tests.

- Keep `canceled_reconnect_preserves_terminal_process`.
  - Retain its real-shell PID and shell-variable assertions.
  - Verify the new protocol never opens a replacement shell during handoff.
- `resume_after_send_before_apply_replays_unapplied_output`
  - Send N, then disconnect before the browser applies it.
  - Resume from N-1. Expect N exactly once in the resumed stream.
- `resume_after_apply_before_ack_does_not_duplicate_output`
  - Apply N, then disconnect before its acknowledgement reaches the server.
  - Resume from N. Expect the next event, not N again.
  - Validate N against the prior attachment's sent prefix.
- `resume_with_evicted_cursor_reports_gap`
  - Resume behind the retained start.
  - Expect Gap and the retained tail. Do not create a new process.
- `resume_wrong_incarnation_requires_fresh_restore`
  - Reuse a terminal ID with a new process incarnation.
  - Expect FreshRestoreRequired. Reject the old snapshot and cursors.
- `resume_missing_process_returns_process_gone`
  - Remove the process before Resume.
  - Expect ProcessGone and zero process-creation calls.
- `ack_applied_is_monotonic_and_attachment_scoped`
  - Duplicate acknowledgements must be harmless.
  - Reject future positions and stale tokens.
  - Assert transport credit cannot underflow or affect another attachment.
- `old_throttle_drop_preserves_new_attachment`
  - Install B's throttle state, then drop A's stream.
  - Assert B's state and acknowledgements remain valid.
- `resize_events_follow_successful_ioctl_order`
  - Interleave output and concurrent resize requests.
  - Verify journal order and emitted dimensions.
  - Failed ioctls must not emit successful Resize events.
- `snapshot_transfer_interruption_can_restart_fresh`
  - Disconnect after a snapshot chunk but before SnapshotEnd.
  - Discard the incomplete restore and restart Fresh.
  - Never expose partial restore as Ready.
- `remote_checkpoint_is_stored_on_process_owner`
  - Route upload through the web-facing server to a remote PTY owner.
  - Verify only the owner stores it.
  - Reattach through the same route and receive that checkpoint.
- `protocol_mismatch_returns_explicit_error`
  - Use incompatible browser and remote protocol versions.
  - Expect rejection. Do not fall back to implicit rewind.

### Serializer and scheduler tests

Location: focused JS tests beside the new snapshot adapter.
Use real pinned XTermJS for round trips. Use a fake clock for scheduling.

- `roundTripPreservesMouseReports`
  - Table-test supported tracking protocols and encodings.
  - Serialize, restore, and inject wheel input.
  - Compare the PTY-bound reports before and after restore.
- `roundTripPreservesSubsequentTerminalBehavior`
  - Table-test the state listed under Serializer compatibility.
  - Feed the same continuation to original and restored terminals.
  - Compare buffers, cursor, modes, and generated input reports.
  - Include alternate-screen exit and saved-cursor restoration.
- `captureWaitsForSafeParserBoundary`
  - Split UTF-8 and CSI/OSC/DCS sequences at every byte boundary.
  - Trigger capture while incomplete. Expect no checkpoint yet.
  - Complete the sequence. Expect a checkpoint at the matching applied cursor.
- `gapResynchronizationPreservesCheckpointModes`
  - Restore mouse modes, then replay a tail starting inside a sequence.
  - Apply the gap adapter. Verify parser recovery and retained mouse modes.
  - Do not assert recovery of commands inside the missing interval.
- `captureUsesAppliedCursorAndMatchingDimensions`
  - Queue output and resize around a pending write callback.
  - Capture only after the callback and ordered resize application.
  - Assert snapshot metadata matches the captured state.
- `captureTriggersAtActivityThreshold`
  - Test 999 versus 1,000 LFs and just below versus at 256 KiB.
  - Include TUI output without any LF characters.
- `captureThrottleKeepsPendingTrigger`
  - Keep output flowing throughout the five-second cooldown.
  - Expect capture at eligibility, without debounce starvation.
  - Assert no two captures occur within five seconds.
- `singleUploadRetainsPendingCapture`
  - Hold an upload open while more output crosses the threshold.
  - Expect one upload in flight and a later eligible capture.
- `failedUploadRetriesWithoutReplacingAcceptedSnapshot`
  - Fail an upload and advance the fake clock through backoff.
  - Verify bounded retries and preservation of the accepted snapshot.
- `snapshotShrinksScrollbackWithoutTruncatingVt`
  - Exceed the byte cap with scrollback.
  - Reduce scrollback and verify a valid round trip.
  - Reject required visible state that still exceeds the cap.
- `staleCallbacksCannotUpdateReplacementAttachment`
  - Replace an attachment with a write or upload pending.
  - Complete old callbacks. Expect no cursor or state changes in the replacement.

### Browser integration tests

#### Mouse-aware TUI fixture

- Add a small Rust fixture binary for terminal integration tests.
- Launch it through the real PTY process path.
- Use the harness's terminal-shell configuration to select the fixture.
- Do not require an installed TUI application.

Fixture startup:

- Set stdin to raw mode. Disable echo and canonical line buffering.
- Enter the alternate screen with `ESC[?1049h`.
- Enable mouse tracking with `ESC[?1000h`.
- Enable SGR mouse encoding with `ESC[?1006h`.
- Draw two panels with fixed, reported cell boundaries.
- Show a scroll counter in each panel.
- Emit a startup-ready event on the test channel.
- Enable mouse modes only once, during startup.
- Never re-enable them during redraw, resize, or reattachment.

Fixture input loop:

- Parse stdin incrementally. Reports may span multiple reads.
- Recognize SGR reports: `ESC[<button;column;rowM`.
- Button 64 means wheel up. Button 65 means wheel down.
- Map the reported cell to its panel.
- Update that panel's counter and redraw it.
- Ignore unrelated keyboard and mouse input.
- Restore termios and leave the alternate screen on normal exit.

Independent observation and control:

- Use a test-only Unix socket on the PTY-owning test server.
- Give each fixture instance a unique socket path and instance ID.
- Have the harness connect through a test helper on that server.
- Send commands and events as newline-delimited JSON.
- Commands include `emit_output`, `disable_mouse`, and `exit`.
- Events include `ready`, `mouse_report`, and `output_complete`.
- Mouse events include raw report bytes, panel ID, and resulting counter.
- Output completion includes a command ID and emitted byte count.
- Keep this channel separate from PTY output and application endpoints.
- Expose it only in the integration-test setup.
- Clean up sockets and processes after each test.

Browser interaction:

- Use Playwright's real wheel input over the XTermJS canvas.
- Derive panel coordinates from terminal geometry and fixture cell boundaries.
- Move the pointer inside a panel, then call `page.mouse.wheel(...)`.
- Do not inject encoded reports directly into stdin or call XTermJS input APIs.
- Wait for a matching `mouse_report` on the independent channel.
- Assert direction and panel. Do not require one report per wheel gesture.
- Also check that the visible panel counter advances.

Snapshot regression sequence:

1. Start the fixture and verify wheel reports before reattachment.
2. Wait for an accepted checkpoint containing the enabled mouse modes.
3. Disconnect the browser output stream.
4. Command the fixture to emit more bytes than the current replay capacity.
5. Wait for `output_complete` and confirm initialization bytes were evicted.
6. Attach a fresh XTermJS and wait for Ready.
7. Send wheel input over the same panel.
8. Assert a new report arrives and the panel counter changes.

- Emit overflow traffic as panel redraws without mode changes.
- This isolates recovery of the checkpoint's mouse state.
- Run a separate case for mode changes lost during overflow.
- Observe checkpoint acceptance through a test hook, not a fixed sleep.
- Keep production snapshot behavior unchanged by that hook.

#### Browser test cases

Add `terminal/tests/integration-test-terminal-snapshot.spec.mjs`.
Use the existing browser test harness and a deterministic PTY fixture.
The fixture reads mouse reports and prints observable counters.
Avoid relying on an installed interactive application.

- `fresh_attachment_preserves_tui_wheel_input`
  - Enable SGR mouse reporting once and accept a snapshot.
  - Produce enough output to evict initialization bytes.
  - Reattach through DOM replacement, then through page reload.
  - Wheel over a panel. Assert the fixture's report counter increases.
  - Disable mouse reporting and exit the alternate screen afterward.
  - Verify subsequent behavior matches those changes.
- `warm_resume_preserves_applied_state`
  - Interrupt the output pipe while keeping XTermJS alive.
  - Reconnect using the applied cursor.
  - Assert no duplicate text, reset, or lost mouse handling.
- `overflow_reports_gap_and_keeps_process_running`
  - Disconnect and write several buffer capacities through the real PTY.
  - Use a bounded external completion marker to prove the writer finished.
  - Reconnect. Assert the dropped-output notice and newest retained marker.
  - Verify the same process PID remains active.
- `dropped_mode_change_is_explicitly_best_effort`
  - Snapshot with mouse enabled, then disable it in output that gets evicted.
  - Restore and verify Gap is visible.
  - Do not claim the restored mode matches the missing disable command.
- `restore_resize_and_remote_routing_work_together`
  - Snapshot at one geometry and reconnect at another.
  - Verify restore geometry precedes the new Resize event.
  - Repeat against the configured remote server.
- `revoked_stream_cannot_feed_replacement_terminal`
  - Hold old stream chunks, replace the attachment, then release them.
  - Assert the replacement ignores old chunks and replies.

### Memory checks and execution

- `replay_storage_stays_bounded_across_many_terminals`
  - Flood multiple disconnected journals.
  - Assert retained payload and metadata counters stay within configured limits.
  - Include slow restores and concurrent snapshot replacement.
  - Account separately for bounded transfer buffers and temporary snapshot copies.
- Run a real-process memory measurement alongside these assertions.
- Record steady-state and replacement peaks.
- Treat allocator RSS as a measurement, not an exact byte assertion.
- Use Bazel for focused testing and the full test suite first.
- Wire PTY, server, JS, and browser tests into Bazel targets.
- Identify exact target labels when adding the tests.
- Run focused targets first with `--test_output=errors --verbose_failures`.
- Run release browser targets with `-c opt`.
- Follow applicable Bazel checks in `terminal/AGENTS.md` afterward.
- Regenerate the JS bundle and check client/server feature builds.

Final validation follows `.github/workflows/merge-validation.yml`:

- Run the full Bazel build and tests.
- Include optimized tests tagged `opt_mode`.
- Once all Bazel tests pass, run the Cargo checks below.
- Cargo tests provide an additional compatibility check.
- The workflow has separate Bazel and Cargo jobs.
- This plan runs them sequentially, with Bazel first.

```sh
export PROTOC=$(which protoc)
bazel build --action_env=PROTOC //...
bazel test --action_env=PROTOC --test_output=errors --verbose_failures --flaky_test_attempts=3 //...
bazel test --action_env=PROTOC --test_output=errors --verbose_failures --flaky_test_attempts=3 -c opt $(bazel query 'attr("tags", "opt_mode", //...)')
bazel run //bazel:buildifier_check
```

Then run the Cargo job's build and test commands:

```sh
cargo build --bins --features=server,server-all,max_level_debug,debug,diagnostics
cargo build --bins --features=server,server-all,max_level_info --release
RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features
./demo/scripts/integration-test.sh target/debug/demo-server
./demo/scripts/integration-test.sh target/release/demo-server
```

Acceptance:

- Correct input behavior with an intact suffix.
- Explicit best-effort recovery after eviction.
- Detached writers continue at the existing history capacity.
- Measured memory includes snapshots and transfer overhead.
- Serializer limitations are documented before default rollout.
