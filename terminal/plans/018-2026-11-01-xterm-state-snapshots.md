# Xterm state snapshots and bounded replay

## Goal

- Preserve TUI state when attaching a fresh xterm.
- Restore mouse handling, screen contents, and supported terminal modes.
- Store a browser-generated snapshot plus a bounded output tail.
- Drop oldest output when the tail is full.
- Keep detached processes running.
- Store recovery state in server memory.
- Add no disk persistence or server-side emulator.

## Current behavior

- `attach.rs` creates a fresh `TerminalJsRc`.
- `javascript.js` creates xterm with default modes.
- Fresh attachment requests `RegisterTerminalMode::Create`.
- For an existing process, the server rewinds its output lease.
- `pty/src/tail.rs` replays only retained byte chunks.
- Older initialization sequences may already be gone.
- The screen can look correct while mouse reporting is disabled.
- Transport reconnect uses `Reopen` and keeps the existing xterm.
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
 xterm <--- ordered events -----+-- bounded output journal <--- PTY reader
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
- Pin a version compatible with the repo's xterm 6.x.
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
- Isolate any private xterm API behind a version-pinned adapter.

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

- Capture only after xterm finishes applying an output event.
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
- Measure desired browser dimensions without immediately resizing xterm.
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
- Advance the applied cursor after xterm's write callback.
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

- Create or reset xterm for Fresh attachment.
- Set the snapshot's original dimensions.
- Restore VT output and supplemental state in the tested order.
- Replay retained events and process any Gap.
- Suppress synthetic restore replies to the PTY.
- Enable input after Ready.
- Request the measured browser size afterward.
- Avoid rendering incomplete restore frames where practical.

## Reconnect flows

```text
Fresh xterm / page reload        Existing xterm / transport reconnect
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
- Resume keeps the existing xterm state.
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

PTY and protocol tests:

- Exact replay with an intact suffix.
- Atomic append, attachment, and checkpoint acceptance.
- Canceled and concurrent lease handoffs.
- Startup without a snapshot.
- Stale tokens, requests, chunks, and acknowledgements.
- Lost upload responses and idempotent retries.
- Oversized appends and bounded metadata.
- Eviction during snapshot transfer and live streaming.
- Disconnect before apply and after apply but before ack.
- Process replacement and missing processes.
- Generation-specific throttle cleanup.

Browser tests:

- Enable SGR mouse once, then exceed the old tail capacity.
- Accept a snapshot before eviction and verify wheel reports after reattach.
- Verify mouse disable and alternate-screen exit afterward.
- Cover page reload, DOM replacement, and warm Resume.
- Cover resize and remote routing.
- Drop a mode change and verify explicit best-effort Gap behavior.
- Test unsafe parser boundaries and split UTF-8.

Scheduler and memory tests:

- Use a controlled clock for triggers and cooldowns.
- Cover continuous output, pending uploads, and upload failures.
- Enforce one capture per five seconds.
- Verify detached writers keep running past buffer capacity.
- Verify oldest output is dropped and memory stays bounded.
- Cover slow readers, repeated gaps, and many terminals.
- Measure snapshot replacement peaks and size rejection.

Build checks:

- Run focused PTY and browser targets first.
- Follow applicable Cargo/Bazel checks in `terminal/AGENTS.md`.
- Regenerate the JS bundle through the existing build process.
- Check client and server feature builds.

Acceptance:

- Correct input behavior with intact history.
- Explicit best-effort recovery after eviction.
- Measured memory use within the planned limits.
- Documented serializer gaps before default rollout.
