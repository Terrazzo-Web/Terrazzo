# Refactor terminal API to `remote_fn`

## Goal

Replace the terminal-specific HTTP API and terminal gRPC service with Terrazzo server functions backed by the generic unary and streaming `remote_fn` macros. The browser-facing functions should follow the same shape as the text-editor API:

```rust
#[server]
pub async fn write(
    remote: ClientAddress,
    request: WriteRequest,
) -> Result<(), ServerFnError> {
    imp::write(remote, request).await
}
```

The server implementation calls a declared remote function, so routing through a remote client no longer requires a terminal-specific gRPC API:

```rust
remote_fn_service::unary::declare_remote_fn!(
    WRITE_REMOTE_FN,
    WRITE,
    WriteRequest,
    (),
    |_server, request| local_write(request),
);
```

Keep terminal behavior unchanged: tabs can be created and restored, input and resize events reach the correct process, titles and ordering persist, output reconnects when a remote connection is interrupted, and closing a tab releases its process and stream.

## APIs being refactored

| API | Current browser transport | Current mesh transport | Target |
| --- | --- | --- | --- |
| `list` | `GET /api/terminal/list` | `TerminalService.List` | unary server function + unary `remote_fn` |
| `new_id` | `POST /api/terminal/new_id` | `TerminalService.NewId` | unary server function + unary `remote_fn` |
| `write` | `POST /api/terminal/write` | `TerminalService.Write` | unary server function + unary `remote_fn` |
| `resize` | `POST /api/terminal/resize` | `TerminalService.Resize` | unary server function + unary `remote_fn` |
| `set_title` | `POST /api/terminal/set_title` | `TerminalService.SetTitle` | unary server function + unary `remote_fn` |
| `set_order` | `POST /api/terminal/set_order` | `TerminalService.SetOrder` | unary server function + unary `remote_fn` |
| `stream/register` | `POST /api/terminal/stream/register` | `TerminalService.Register` server stream | streaming server function + streaming `remote_fn` |
| `stream/close` | `POST /api/terminal/stream/close` | `TerminalService.Close` | unary `remote_fn`, or stream-drop cleanup when equivalent |
| `stream/ack` | `POST /api/terminal/stream/ack` | `TerminalService.Ack` | remove if generic streaming backpressure supersedes the terminal window; otherwise unary `remote_fn` |
| `stream/pipe` | multiplexed HTTP response body | none | remove; consume the server-function stream directly |
| `stream/pipe/keepalive` | HTTP keepalive | none | remove; rely on generic streaming transport lifecycle |
| `stream/pipe/close` | HTTP pipe cleanup | none | remove; dropping/cancelling the server-function stream performs cleanup |

The `set_tile_id` remote function already uses the target unary pattern and is not part of the old terminal HTTP/gRPC API migration.

## Task 1: Define the server-function surface

1. Introduce a terminal API module alongside the terminal feature code, following `text_editor/fsio` and `logs/stream`:
   - public `#[server]` functions compiled for the browser and server;
   - a server-only implementation module containing request types and `declare_remote_fn!` registrations;
   - stable remote-function names for every operation.
2. Prefer the shared serializable terminal schema types over protobuf conversions. Add small request structs only where a function has multiple arguments or needs server context.
3. Pass the destination as `ClientAddress`/`Remote` explicitly to each public function. Derive it from `TerminalAddress::via` at call sites when the operation targets an existing terminal.
4. Return `ServerFnError` at the public boundary and preserve useful status categories (especially terminal-not-found and remote-disconnected) when converting local errors.
5. Keep local implementations separate from dispatch wrappers so each declared callback executes the operation against the `Server` supplied by `remote_fn` without recursively routing it.

## Task 2: Migrate unary operations

1. Implement and register unary remote functions for `list`, `new_id`, `write`, `resize`, `set_title`, `set_order`, and `close`.
2. Decide the scope of `set_order` explicitly. The current request may contain terminal addresses on different remotes; group addresses by `via` and issue one remote call per destination, or enforce and validate a single destination. Preserve the UI's visible ordering semantics in either case.
3. Move the title/ID construction currently in the Axum `new_id` handler into the shared server implementation, including client-name fallback and concise-trace ID behavior.
4. Preserve list sorting by `order` and the current local-client-name behavior.
5. Update terminal UI call sites to invoke the server functions and remove their dependency on `api::client::terminal_api::{list,new_id,write,resize,set_title,set_order}`.
6. Add focused tests that call each function locally and through a configured remote server. Cover missing terminals, routing to the requested remote, and serialization of the shared schema.

## Task 3: Migrate terminal output streaming

1. Declare terminal registration as a streaming remote function whose input contains the terminal definition and create/reopen mode and whose items contain terminal output bytes (or an explicit end-of-stream item when needed).
2. Expose it through a streaming `#[server]` function, using the same `Http<Json, StreamingText>`/NDJSON approach as log streaming unless a binary server-function codec is already available and supported in the browser.
3. Move registration ownership and cleanup into the returned stream. A guard owned by the local stream must unregister the listener and close the process when the browser cancels or drops it, matching current end-of-stream behavior.
4. Preserve reconnect semantics: distinguish a temporarily disconnected remote transport from process exit, and reopen an existing terminal without creating a second process.
5. Replace the client `pipe`, dispatcher map, correlation IDs, keepalive loop, chunk framing, and manual stream reader with direct consumption of the server-function stream.
6. Evaluate acknowledgement after the streaming path works:
   - remove `AckRequest`, `STREAMING_WINDOW_SIZE`, and `ack` if the generic stream provides bounded backpressure end to end;
   - otherwise register `ack` as a unary remote function and retain the window tests until equivalent flow control is demonstrated.
7. Add tests for initial output, multiple concurrent terminals, reconnect, normal process exit, browser-side cancellation, remote disconnect, and cleanup without a final explicit close request.

## Task 4: Remove the dedicated transports

1. Remove the nested `/api/terminal` Axum router and all client request helpers under `api/client/terminal_api` once all call sites use server functions.
2. Remove `terminal_service/grpc.rs`, `TerminalServiceServer` registration, generated `TerminalServiceClient` usage, and the terminal-specific forwarding implementations.
3. Remove `terminal.proto` and its build registration after moving every still-needed domain type to Rust/Serde types. Keep protobuf types only if another non-terminal consumer remains.
4. Remove obsolete correlation-ID headers, keepalive constants, request/error types, conversions, dependencies, Bazel inputs, and feature gates. Use `rg` to verify that no `/api/terminal`, `TerminalService`, or terminal proto references remain.
5. Keep the generic `RemoteFnService` gRPC service: it is the single mesh transport used by the new unary and streaming registrations.

## Task 5: Validate

1. Run formatting and focused unit tests for the terminal API, remote dispatch, and stream lifecycle.
2. Build client/server feature combinations to catch `cfg(feature = "client")`, `server`, `terminal`, diagnostics, and production-client differences.
3. Run the existing terminal integration tests, paying particular attention to:
   - creating, restoring, reordering, and closing tabs;
   - writing and resizing;
   - title updates;
   - simultaneous output streams;
   - remote reconnect and process cleanup.
4. Run the repository validation relevant to the touched targets:

   ```sh
   cargo build --bins --features=server,server-all,max_level_debug,debug,diagnostics
   RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features
   bazel build //terminal/...
   bazel test --test_output=errors --verbose_failures //terminal/...
   bazel run //bazel:buildifier_check
   ```

## Suggested commit sequence

1. Add terminal server functions and unary remote-function registrations with tests.
2. Switch unary browser call sites and remove their HTTP routes.
3. Add the streaming remote function and stream-lifecycle tests.
4. Switch browser output handling and remove the multiplexed pipe protocol.
5. Remove `TerminalService`, `terminal.proto`, and remaining dead transport code.

Do not remove an old route or RPC in a commit until its replacement is covered and all in-tree call sites have migrated; this keeps each step reviewable and bisectable.
