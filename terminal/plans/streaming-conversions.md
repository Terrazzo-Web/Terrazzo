# Stream converter results incrementally

## Summary
Change the converter server function from a single JSON response that contains all conversions to an HTTP stream of newline-delimited JSON conversion records, following the same `StreamingText` pattern already used in `terminal/src/logs/stream.rs`.

This planning pass does not change runtime behavior yet. It captures the implementation path so the streaming work can be done in a follow-up without re-discovering the current seams.

## Status Quo
- `terminal/src/converter/api.rs` exposes `get_conversions(...)` as `Http<Json, Json>` and returns `Conversions`.
- `terminal/src/converter/service.rs` collects every `Conversion` into a `Vec` before returning.
- `terminal/src/converter/ui.rs` issues one debounced request and replaces the whole `Conversions` state when the response arrives.
- The repo already has a working streaming server-function example in `terminal/src/logs/stream.rs` using `Http<Json, StreamingText>` and `TextStream<ServerFnError>`.
- The repo already has reusable client-side NDJSON parsing logic in `terminal/src/logs/client/ndjson.rs`.
- Existing browser coverage for the converter lives in `terminal/tests/integration-test-converter.spec.mjs`.
- Current repo usage indicates the only caller of `get_conversions(...)` is the converter UI.

## Desired Behavior
- The converter endpoint should stream one serialized conversion at a time instead of buffering the full list first.
- The wire format should be easy for the browser client to parse incrementally. NDJSON over `StreamingText` is the best fit with the existing logs implementation.
- The UI should be able to clear stale results when a new request starts, append each streamed conversion as it arrives, and still handle request failures cleanly.
- Conversion ordering no longer needs to be deterministic once results are streamed. Converters may publish results whenever they are ready.
- The client should treat HTTP response completion as the signal that no more conversions are pending. No explicit terminal “done” event is required.

## Key Changes
- [ ] Update `terminal/src/converter/api.rs`:
  - Switch the server function protocol from `Http<Json, Json>` to `Http<Json, StreamingText>`.
  - Change the return type from `Conversions` to `TextStream<ServerFnError>`.
  - Add a small server-only implementation module, mirroring `terminal/src/logs/stream.rs`, that converts conversion items into NDJSON lines.
- [ ] Refactor `terminal/src/converter/service.rs` so conversion generation can stream:
  - Extract an async stream-oriented path instead of always collecting into `Vec<Conversion>`.
  - Since the only caller is the UI, optimize the public path for streaming instead of preserving the current collected API shape for other consumers.
  - Allow converters to emit results as they become available rather than preserving today’s aggregate ordering.
- [ ] Introduce a serialized stream payload:
  - Reuse `Conversion` if it serializes cleanly per item, instead of inventing a wrapper event type prematurely.
  - Serialize each item as one JSON object followed by `\n`.
  - Keep the payload data-only. Stream completion already tells the client there are no more pending conversions.
- [ ] Update `terminal/src/converter/ui.rs`:
  - Replace the single `get_conversions(...)` await with incremental stream consumption.
  - Reset the `Conversions` signal before reading a new stream so old tabs disappear immediately.
  - Refactor and share the client-side chunk buffering/parsing logic currently in `terminal/src/logs/client/ndjson.rs` instead of duplicating NDJSON parsing in converter UI code.
  - Append each decoded `Conversion` to the existing state in arrival order.
  - Guard against out-of-order debounced requests so an older stream cannot overwrite a newer input.
- [ ] Refactor shared NDJSON helpers:
  - Extract a generic client-side NDJSON buffer/parser from `terminal/src/logs/client/ndjson.rs` so both logs and converter streaming can reuse it.
  - Extract a generic server-side “serialize item as NDJSON line” helper from the `serialize_log_event` logic in `terminal/src/logs/stream.rs`.
  - Keep the logs behavior unchanged after the shared helper is introduced.

## Implementation Notes
- `terminal/src/logs/stream.rs` is the reference shape for:
  - `StreamingText`
  - `TextStream<ServerFnError>`
  - `map_ok(...)` plus newline-delimited serialization
- `terminal/src/logs/client/ndjson.rs` is the reference shape for incremental client-side NDJSON parsing and should be generalized rather than copied.
- `terminal/src/converter/service.rs` currently uses:
  - `add_conversions(input, &mut add)` as the single place that defines conversion order
  - a synchronous callback for most conversions plus async checks for TLS info and DNS
- The lowest-risk follow-up is likely:
  - keep `add_conversions(...)` as the place that schedules converter work
  - write streamed results into a channel from async work
  - expose the receiver as a stream
- Because ordering is no longer deterministic, UI expectations and tests should assert presence/content, not a fixed tab order, unless the implementation later restores ordering intentionally.
- If boxing the stream is simpler than threading generic stream types through the API, follow the logs example and return a boxed/pinned stream.

## Open Questions
- Does the browser-side `#[server]` client generated by the current stack expose a convenient text stream reader, or will the UI need a lower-level fetch path for this endpoint?

## Test Plan
- [ ] Add or update unit tests around the shared client-side NDJSON parser after refactoring `terminal/src/logs/client/ndjson.rs` to be reusable for both logs and conversions.
- [ ] Add unit tests around the shared server-side NDJSON serialization helper extracted from `serialize_log_event`.
- [ ] Server-side unit test that the stream yields the expected conversion rows without requiring a deterministic order.
- [ ] Server-side unit test that each emitted chunk is valid newline-delimited JSON.
- [ ] UI/client test that a new request clears old results, then appends conversions incrementally.
- [ ] UI/client test that a superseded request cannot repopulate the state after a newer request starts.
- [ ] Keep and extend `terminal/tests/integration-test-converter.spec.mjs` for browser-level coverage of the streaming converter behavior.
- [ ] Add at least one integration test in `terminal/tests/integration-test-converter.spec.mjs` that verifies streamed results appear successfully for a known input without assuming fixed conversion order.

## Validation
- Follow-up implementation should run the smallest relevant Rust test target touching converter behavior.
- Run `bazel test //terminal:converter-integration-test-release` for browser-level validation. This target runs `terminal/tests/integration-test-converter.spec.mjs`.
