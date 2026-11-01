# Xterm state snapshots and lossless reconnect

## Goal

Restore an existing terminal's screen and input modes when a new browser xterm
attaches, including TUI mouse scrolling. Replace replay of an arbitrary bounded
output tail with a browser-generated checkpoint plus every subsequent output
event. Keep snapshots in memory on the server owning the PTY; do not introduce a
server-side terminal emulator or disk persistence in this change.

This document is an implementation plan, not an implemented protocol. Use the
filename date as the intended commit date; update it if committed later.

## Current behavior and failure

- `terminal/src/terminal/attach.rs` constructs a new `TerminalJsRc` when attaching
  a new element. `javascript.js` constructs a default xterm.
- `terminal/src/terminal/client.rs` starts with `RegisterTerminalMode::Create`.
  For an existing process, `terminal/service/stream.rs` requests a rewind through
  `processes/stream.rs` and `pty/src/lease.rs`.
- `pty/src/tail.rs` stores bounded raw byte chunks and evicts the oldest chunks.
  Rewind starts at the oldest remaining chunk, without reconstructing parser,
  buffer, or mode state. A TUI's initial mouse enable sequence can be gone.
- A transport reconnect uses `Reopen` and retains the browser xterm, but relies
  on the server lease cursor rather than an explicit browser-applied position.
- `STREAMING_WINDOW_SIZE` currently serves both transport throttling and raw
  replay capacity. These become independent settings.

## Architecture and invariants

```text
Browser                              PTY-owning server (local or remote)
                                     ProcessIoEntry
  xterm <--- ordered events ---------+-- replay journal <--- PTY reader
    |                               |       |
    +-- serialize after apply ------+       +-- grows while disconnected
          checkpoint upload         |
                                    +-- latest accepted checkpoint

Fresh attach: [checkpoint at N] [events N+1 ... H] [live events H+1 ...]
Warm resume:                   [events applied+1 ... H] [live ...]
```

1. Output is never silently skipped. A checkpoint covers an exact event prefix;
   reconstruction uses all events after that prefix exactly once.
2. Only the current attachment may write, resize, acknowledge, or upload a
   checkpoint. Process incarnation and attachment tokens fence stale requests.
3. Snapshot acceptance and prefix compaction are atomic with respect to journal
   append, lease handoff, and attach initialization.
4. Transport acknowledgements free transport credit; only accepted checkpoints
   allow compaction of replay history. These are separate operations.
5. PTY processing remains independent of the browser, until the explicit replay
   memory limit applies backpressure. Closing a stream does not close the PTY.

## First milestone: establish serializer correctness

Add a pinned `@xterm/addon-serialize` version compatible with the repo's xterm
6.x dependency in `terminal/assets/jsdeps/`, and export it through `JsDeps`.
Keep modes and the alternate buffer included. Serialize a configured amount of
normal-buffer scrollback (initially 1,000 rows), as well as the visible screen.
This limit is independent of the snapshot trigger interval.

The addon generates VT output, not a complete dump of xterm internals. Before
building the protocol, add round-trip fixtures against the pinned versions:

- Mouse protocols 9/1000/1002/1003 and encodings 1005/1006/1015/1016 where supported;
  verify emitted wheel reports, not just visual equality.
- Alternate screen entry/exit and saved cursor, scroll margins, cursor visibility,
  application cursor/keypad modes, bracketed paste, origin/wrap modes, tab stops,
  character sets, current text attributes, wide characters, and wrapped lines.
- UTF-8 characters and CSI/OSC/DCS sequences split across arbitrary chunks.

Audit and explicitly supplement missing state. In particular, current upstream
mode serialization enables mouse tracking but does not serialize mouse encoding.
Track missing state through parser hooks where available, otherwise a small,
version-pinned adapter with focused tests. Do not infer modes from screen text.
If a necessary state cannot be captured and restored reliably, stop rollout and
document the limitation; the stock addon alone is not proof of correctness.

Snapshots must also have a parser-safe boundary: no pending UTF-8 decoder bytes
or partial control sequence. A completed `write` callback alone does not prove
this. The adapter must identify safe boundaries; if an output chunk ends inside
a sequence, defer the checkpoint until a later safe boundary. The journal keeps
the bytes meanwhile. Bound incomplete control-string handling consistently with
xterm and test it. Do not discard a prefix whose parser continuation is missing.

References:
- https://github.com/xtermjs/xterm.js/blob/master/addons/addon-serialize/typings/addon-serialize.d.ts
- https://github.com/xtermjs/xterm.js/blob/master/addons/addon-serialize/src/SerializeAddon.ts

## PTY crate API changes

Replace the terminal's use of the evicting `TailStream` with an ordered replay
journal. Keep xterm-specific snapshot contents opaque to `terrazzo_pty`.
Names below are proposed API shapes, not final Rust signatures.

```text
ProcessIoEntry
  incarnation: unpredictable process identity
  active_attachment: server-issued token
  journal: ReplayJournal
    checkpoint: Option<CheckpointEnvelope>
    base_event: EventId
    next_event: EventId
    output_end: ByteOffset
    events: VecDeque<JournalEvent>
    retained_bytes / capacity / wakeups
  input: existing PTY writer

JournalEvent = Output { event_id, start_byte, bytes }
             | Resize { event_id, size }

CheckpointEnvelope = { event_id, output_end, size, format_version, payload }
```

Use monotonically increasing event IDs for the replay cursor and absolute byte
offsets for output accounting. Resize events require an event cursor because
multiple resizes may occur without any output. On the wire encode 64-bit cursors
as decimal strings (or another lossless representation), avoiding JavaScript's
integer precision limit. Tokens and cursors are scoped to a process incarnation.

Proposed operations:

- `attach(Fresh | Resume { incarnation, applied_event })`: revoke the prior
  lease, issue an attachment token, and return a stream with an immutable restore
  descriptor followed by an exact journal suffix and live events. Fresh may
  create a missing process only when explicitly allowed by the terminal service.
- `commit_checkpoint(token, envelope)`: validate current ownership, format,
  monotonic position, size, payload limits, and that the position was sent and
  acknowledged as applied by this attachment; atomically install and compact.
  The server validates protocol metadata, not the semantic truth of VT payloads.
- `ack_applied(token, event_id, output_end)`: monotonically acknowledge a sent
  prefix, never beyond what this attachment received.
- `resize(token, size)`: serialize resize requests and journal the successfully
  applied dimensions. Expose an explicit starting size when opening a process.

Replace `lease_output(rewind: bool)` in the terminal path with these explicit
attach semantics. Preserve cancellation-safe exchange of `ProcessOutputLease`:
a canceled handoff must not lose the receiver or replace the process. Adapt
`LeaseItem::Data` to carry cursor metadata and add resize/initialization items.
Inventory other PTY consumers before removing any old API; retain a compatibility
wrapper if necessary, but do not route terminal restore through lossy rewind.

## Resize ordering

The journal coordinator serializes reader appends, resize ioctls, lease changes,
and snapshot commits. Record each successful resize after the ioctl and before
accepting the next output append. This defines the emulator's authoritative
ordering; bytes already in the OS PTY buffer cannot be assigned an exact causal
time relative to the ioctl. Match the browser to the journal order rather than
attempting to infer that causal time.

Stop letting `fit()` immediately mutate the browser terminal independently of
this ordering. Measure desired dimensions, submit a resize, and apply the emitted
Resize event in the same queue as Output events. Serialize concurrent resize
requests. Remove the artificial shrink/sleep/grow resize used to force redraw
on attach. Fresh process creation supplies initial dimensions before spawning
where possible. Test interactive resizing for latency and correct redraw.

## In-memory retention and limits

Store all recovery state inside the existing PTY-owning `ProcessIoEntry`, shared
by the global process map. Do not put authoritative state in the gateway's pipe
or per-request throttling map. Remote API calls forward to the owning server.
State lives only as long as the process entry; close/EOS cleanup releases both
checkpoint and journal, and server restart loses them with the PTY session.

Use immutable shared byte buffers for checkpoints and event payloads. A restore
stream pins the descriptor and suffix it needs while newer snapshots are
committed. Count pinned buffers and in-flight checkpoint replacement against
memory budgets; compaction must not invalidate a reader or hide memory use.

Suggested initial tunables, to validate under load:

- Snapshot payload limit: 8 MiB per terminal.
- Post-checkpoint journal limit: 16 MiB per terminal, plus bounded event metadata.
- A configurable global recovery budget, initially 256 MiB; reserve capacity
  before reads/allocations, including snapshot upload and replacement overhead.
- Independent existing transport window; snapshot transfer uses bounded chunks
  and its own acknowledgement accounting.

When journal capacity is exhausted, pause PTY reads rather than evict data. The
OS PTY buffer eventually fills and blocks the producing application. This also
applies while no browser is connected: a noisy detached job may pause until a
browser reconnects, consumes the suffix, and commits a checkpoint. Expose this
state in diagnostics. Reconnect/restore and checkpoint acceptance must remain
possible at the limit; reserve bounded control/upload capacity and prioritize
compaction so the recovery path cannot deadlock.

This is a deliberate behavioral change from lossy history. If uninterrupted,
arbitrarily long detached output is required, a browser-only in-memory design
cannot provide it with bounded memory. That requires disk spooling or a
continuously running server emulator, which is outside this plan.

Before the first checkpoint, retain events from process start. A rejected or
failed upload leaves the previous checkpoint and its entire suffix intact.

## Terminal API and remote protocol changes

Replace `RegisterTerminalMode::{Create, Reopen}` with a versioned attach request
carried through `api::stream`, `add_stream`, stream registration, and remote
`terminal.stream`. Separate permission to create a process from restore strategy.

```text
AttachRequest {
  protocol_version,
  terminal_def,
  create_if_missing,
  restore: Fresh | Resume { incarnation, applied_event, output_end }
}

Server stream:
  Init { incarnation, attachment, strategy, base_event, size, format_version }
  SnapshotStart { total_bytes, checkpoint metadata }   # Fresh, if available
  SnapshotChunk { index, bytes }
  SnapshotEnd
  Output { event_id, start_byte, bytes: Utf8 | Base64 }
  Resize { event_id, size }
  Ready { replay_through }
  ...live Output / Resize...
  Eos | Revoked | Error { code, details }
```

Keep current UTF-8/base64 transport choices but preserve exact original bytes
and lengths. Snapshot chunks are synthetic VT output, so they do not advance PTY
byte offsets or receive ordinary output acknowledgements. Add bounded restore
transfer credit to avoid buffering a whole snapshot in the multiplexed pipe.

`Ready` marks the captured replay high-water point, not a promise that the PTY
has stopped producing output. Lease creation atomically captures that point and
registers the suffix/live cursor; no gap exists between history and live output.

Add `put_checkpoint(terminal, incarnation, attachment, checkpoint)` as an
authenticated server function plus a new remote unary function on the same
routing path as resize/ack. Return `Accepted { event_id }`, `AlreadyAccepted`,
or structured rejection (`StaleAttachment`, `UnsupportedFormat`, `InvalidCursor`,
`TooLarge`). Permit retry after a lost response without changing state twice.

Change `ack` from a byte count to an incarnation/token-scoped cumulative applied
cursor. Track sent and acknowledged output positions per attachment, validate
bounds and monotonicity, and derive outstanding transport bytes. Fix throttle
cleanup to remove only its own generation; an old stream's drop must not delete
a replacement stream's entry. Scope write and resize requests to the same token.

Include attachment identity in pipe dispatch/registration or equivalent fencing
so late chunks from a revoked stream cannot enter the replacement xterm. The
existing terminal-ID-only multiplexing is insufficient during overlapping leases.

Keep the global terminal list/title model. Snapshot payloads use existing session
authorization and remote ownership checks; impose request limits before body
allocation where supported. Log sizes/cursors, never terminal contents.

## Browser snapshot scheduling and application queue

Use one queue per attachment for Output, Resize, restore, and checkpoint capture.
Await xterm's output write callback before advancing the applied cursor. Take a
snapshot synchronously between queue entries at a parser-safe boundary, capture
its cursor and size in the same step, then upload asynchronously. Allow only one
upload in flight; continue applying output while it runs. Retain pending work
if a trigger occurs during upload, and ignore stale completion callbacks.

Trigger after 1,000 processed LF characters OR 256 KiB processed output since
the last captured checkpoint, throttled to at most one capture every five seconds.
LF count is an inexpensive output-activity metric, not an assertion about screen
rows; the byte trigger covers TUIs that redraw without line feeds. Once triggered,
schedule the earliest eligible capture rather than resetting a debounce timer
on every byte. Continuous output must not starve snapshots. Capture an initial
checkpoint after initial restore/replay even below the thresholds (respecting
the throttle), so quiet sessions establish a baseline. Retry failed uploads with
bounded backoff. Never make unload-time uploads a correctness requirement.

Capture actual terminal dimensions and a versioned supplemental-state envelope.
Use fresh/reset xterm state for restore, install original geometry, apply the
VT snapshot and supplemental state in the tested order, then replay events.
Keep synthetic restore-generated input/replies disconnected from PTY input.
Enable live input after `Ready`; apply newly measured geometry through an ordered
resize request afterward. Avoid opening/rendering intermediate restore frames
where practical. Preserve the existing title metadata instead of relying on VT
serialization to reconstruct it.

## Reconnect flows

```text
New xterm / page reload               Existing xterm / interrupted transport
----------------------               -------------------------------------
Fresh attach                         Resume(incarnation, last applied cursor)
  -> Init + checkpoint                 -> new attachment token
  -> reset, restore original size      -> replay only unapplied events
  -> replay exact suffix               -> Ready
  -> Ready                             -> resume input
  -> input + requested resize
  -> initial/scheduled checkpoint
```

Fresh attach without a checkpoint replays from process start at its initial size.
Resume never resets or replays a snapshot onto an already populated terminal.
If its cursor is no longer retained or incarnation differs, return an explicit
`FreshRestoreRequired`; the browser then creates/reset-restores an xterm using
Fresh. Do not silently jump forward. A missing process during Resume returns
`ProcessGone`, never automatically starts a shell under an existing display.

Maintain an applied cursor only after the matching write callback completes.
On transport loss, finish or cancel queued work consistently before constructing
Resume; the request must match the actual retained xterm. On replacement,
generation-check every async callback. `Revoked` ends ownership rather than
triggering automatic reacquisition on mouse activity and fighting another tab.

## Implementation sequence

1. Build pinned serializer/adapter round-trip fixtures, including safe boundaries
   and SGR mouse encoding. Resolve coverage gaps before adopting checkpoints.
2. Add journal/event IDs, independent replay budgets, backpressure, checkpoint
   commit and cancellation-safe attach APIs in `pty/`. Preserve existing callers
   until the terminal migration is complete.
3. Extend terminal schema, remote functions, lease envelopes, cumulative acks,
   resize ordering, and generation-scoped pipe/throttle cleanup together.
4. Implement browser ordered application, Fresh/Resume restore, supplemental
   state, throttled capture, checkpoint uploads, and stale callback fencing.
5. Remove terminal rewind fallback and artificial resize redraw; document memory
   limits and detached-output backpressure. Roll out only with matched protocol
   versions. Reject mismatched clients/remote servers explicitly rather than
   silently reverting to the state-losing path.

Primary files: `pty/src/{lib,tail,lease}.rs`, new replay journal module;
`terminal/src/processes/{mod,stream,resize}.rs`;
`terminal/src/api/shared/terminal_schema.rs`;
`terminal/src/terminal/{api,client,attach,javascript}.rs` and `javascript.js`;
`terminal/src/terminal/service/` and `streams/`;
`terminal/src/backend/throttling_stream.rs`; JS dependency sources/lockfile.

## Validation and acceptance

- Journal tests: exact suffix reconstruction, atomic checkpoint/append/attach,
  compaction with pinned readers, canceled/concurrent handoffs, startup without
  checkpoint, stale tokens, upload retries, and memory-bound backpressure/wakeup.
- Protocol tests: disconnect after send but before apply; disconnect after apply
  but before ack; new incarnation; cursor outside retention; stale chunks/acks/
  writes/resizes; old throttle drop; snapshot transfer across pipe interruption.
- Browser regression: fixture enables SGR mouse once, emits more than the old
  tail capacity, reattaches, and still receives wheel reports in its panel.
  Verify mouse disable and alternate-screen exit afterward. Repeat with reload,
  DOM replacement, warm transport resume, resize, and remote routing.
- Scheduler tests with controlled clock: high-rate output, fewer than 1,000 LFs,
  pending trigger during cooldown/upload, unsafe parser boundary, upload failure,
  quiet initial checkpoint, and no more than one capture per five seconds.
- Resource tests: disconnected writer reaches the configured limit without data
  loss or unbounded memory; reconnect restores and checkpoint frees capacity.
  Exercise many terminals, replacement snapshot peaks, and a globally full budget.
- Run focused PTY and terminal browser targets first, then applicable Cargo/Bazel
  validation from `terminal/AGENTS.md`. Regenerate the JS bundle with the repo's
  existing build process and check both client and server feature builds.

Acceptance requires correct input behavior and subsequent terminal evolution,
not merely a visually similar restored screen. Report serializer limitations
and snapshot/retention measurements before enabling the protocol by default.
