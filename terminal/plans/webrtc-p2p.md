# WebRTC peer-to-peer gateway transport

## Goal

Allow a Terrazzo client to call a Terrazzo server that is behind NAT without the
server exposing a public listening address. A public `trz_gateway_server` is the
signaling node only: it introduces peers and relays WebRTC negotiation messages,
but application HTTP traffic flows directly between the client and server.

The first end-to-end milestone is certificate enrollment. Given only the
signaling URL, the target server's globally unique
`trz_gateway_common::id::ClientName`, and the target server's PKI configuration,
`trz_gateway_client` must obtain `/remote/certificate` through WebRTC. The client
must never learn or connect to the target server's TCP listening port.

## Proposed topology

```text
                         public HTTPS/WSS
  NATed Terrazzo Server --------------------> Signaling Gateway
       (registered as A)                     /p2p/register/A
             ^                                      ^
             |                                      |
             | ordered, reliable WebRTC             | public HTTPS/WSS
             | data channel                         |
             +---------------- Terrazzo Client ------+
                                /p2p/connect/A

  HTTP/1.1 or HTTP/2 + the target server's TLS run inside the data channel.
  SDP and trickled ICE candidates are the only payloads relayed by signaling.
```

Use the Tokio runtime support in the [`webrtc` crate](https://docs.rs/webrtc).
Configure `stun:stun.l.google.com:19302` initially. STUN permits direct
connectivity through compatible NATs; it is not a relay, so TURN configuration
must be supported by the model even though deploying a TURN server is not part
of the first milestone.

Each HTTP transport connection gets one WebRTC peer connection and one data
channel. Reqwest connection pooling and HTTP/2 can reuse it. This keeps the byte
stream and shutdown model simple; sharing one peer connection across several
independent HTTP connections can be considered after correctness is established.

## Transport invariants

- Create the data channel explicitly as ordered and reliable: `ordered = true`,
  with neither a maximum retransmit count nor a maximum packet lifetime. SCTP
  then retransmits and preserves order; this must not be configured as a lossy
  UDP-style channel.
- Adapt the message-oriented data channel to a continuous Tokio
  `AsyncRead + AsyncWrite` byte stream. Writes are split into bounded binary
  messages, reads concatenate messages into a buffer, send-buffer pressure is
  awaited, and close/error events become EOF or an `io::Error`.
- Run the target server's existing TLS handshake and HTTP service over that byte
  stream. The signaling server never terminates or sees application TLS, auth
  codes, certificates, HTTP headers, or response bodies.
- Preserve the current direct TCP/WebSocket transports. P2P is selected by
  configuration and can be rolled out without changing existing deployments.
- Put connection IDs, timeouts, frame-size limits, state transitions, and peer
  names in tracing spans. Never log SDP, ICE credentials, auth codes, or HTTP
  bodies.

## Signaling protocol

Add serializable signaling types in a shared `p2p` module. At minimum they need:

- `P2pConnectionId`, allocated unpredictably by the signaling node;
- an SDP offer and answer;
- trickled ICE candidates and an explicit end-of-candidates marker;
- connection cancellation and structured failure messages.

Expose two WebSocket endpoints from `Server::make_app`:

```text
GET /p2p/register/{server_name}
GET /p2p/connect/{server_name}
```

`register` is a persistent, outbound control connection from the NATed server.
Its lifetime is the registration lifetime. `connect` is opened by a client for
one attempted peer connection. The signaling node allocates a connection ID,
associates it with the requested `ClientName`, and relays only messages carrying
that ID between the two WebSockets. Use a typed JSON envelope initially because
the control traffic is small and diagnosability matters more than encoding size.

The signaling registry should be a component owned by `trz_gateway_server::Server`
and backed by concurrent maps:

- one active registration per `ClientName`;
- registration waiters keyed by `ClientName` so clients can arrive before servers;
- pending sessions keyed by `P2pConnectionId`;
- bounded per-peer queues and a short handshake deadline;
- cleanup on either WebSocket closing, timeout, server shutdown, or explicit
  cancellation.

A second registration for the same name atomically replaces the first. Cancel
the old registration's pending sessions and close its control WebSocket so it
cannot continue relaying messages. Associate cleanup with a registration
generation/token so the displaced connection's close handler cannot remove its
replacement.

`/p2p/connect/{server_name}` waits for up to 30 seconds for that server to
register. Wake all waiters when registration succeeds, then bind each request to
the current registration generation. Return 404 only when the 30-second wait
expires; return 429 when per-peer/global pending-session limits are reached, and
close malformed or oversized signaling streams. Client cancellation must remove
its waiter immediately. ICE candidates may arrive before the remote SDP is
installed, so each peer must queue them until `set_remote_description` succeeds.

`ClientName` is a routing key, not proof of identity. End-to-end TLS prevents a
signaling node or a name hijacker from reading or impersonating the target HTTP
server, but an unauthenticated replacement registration can cause denial of
service. Before enabling this API on an untrusted public gateway, add an
`AppConfig`/`GatewayConfig` registration-authorization hook (mTLS, bearer token,
or a deployment-specific policy). Keep that decision separate from the WebRTC
wire protocol so the local integration test can use an allow-all policy.

## Task 1: Shared WebRTC connection layer

1. Add a pinned `webrtc` workspace dependency with Tokio runtime support and
   update Cargo and Bazel dependencies for the common, client, and server targets
   that use it.
2. Add shared signaling DTOs and validation. Limit SDP/candidate lengths, reject
   unknown connection IDs, and make protocol versioning explicit in the initial
   registration message.
3. Implement a `DataChannelIo` adapter around the crate's data-channel callbacks.
   Use bounded channels between callbacks and Tokio I/O, preserve partial-read and
   partial-write semantics, flush against the data-channel send buffer, and make
   close idempotent.
4. Add unit tests for message fragmentation/coalescing, backpressure, EOF,
   simultaneous close, and propagation of data-channel errors. The tests should
   not need external networking.
5. Add a small peer-connection builder shared by both roles. It accepts ICE server
   URLs, emits local candidates to a signaling sink, applies remote candidates in
   order, and resolves only after the reliable data channel is open.

Keep the transport implementation in the focused `trz_gateway_common::p2p`
module rather than duplicating WebRTC callback/state code in
`trz-gateway-client` and `trz-gateway-server`. This deliberately keeps the shared
surface in the existing common crate instead of introducing another crate.

### Task 1 implementation log (2026-08-30)

- Added an exact `webrtc = 0.20.3` workspace dependency with only its Tokio
  runtime feature, enabled the supporting Tokio features in
  `trz-gateway-common`, and updated the Cargo and generated Bazel module lock
  files. Bazel consumes the same common manifest through `all_crate_deps`, so no
  handwritten external Bazel dependency list was needed.
- Added `trz_gateway_common::p2p::protocol` with a UUID-backed
  `P2pConnectionId`, protocol-versioned `Hello`, offer/answer, candidate,
  end-of-candidates, cancel, and structured failure messages. Validation bounds
  SDP, candidate, URL, and failure strings and provides an active-connection
  check that rejects unknown IDs.
- Added `trz_gateway_common::p2p::data_channel_io::DataChannelIo`. It exposes a
  WebRTC data channel as `AsyncRead + AsyncWrite`, joins incoming message
  boundaries into a byte stream, splits writes into 16 KiB binary messages, uses
  bounded read/write queues, waits for the crate's bounded send path, makes flush
  ordering cancellation-safe, and maps close/error/send failure into Tokio EOF or
  `io::Error`. Local shutdown is idempotent.
- Added `trz_gateway_common::p2p::peer_connection`. The shared builder accepts
  STUN/TURN definitions and UDP bind addresses, emits candidates plus an explicit
  end marker to a bounded signaling sink, queues candidates received before SDP,
  applies them serially in arrival order, and configures a 16 MiB WebRTC send
  buffer. Both created and accepted channels are rejected unless they are ordered
  with unlimited retransmission/lifetime; `PendingDataChannel::wait_open` returns
  the byte stream only after the WebRTC open event.
- Added hermetic tests for protocol round-trips and validation, fragmented and
  coalesced I/O, bounded backpressure, flush/send errors, EOF, idempotent
  simultaneous close, and error propagation. A real two-peer test exchanges SDP
  and trickled host ICE candidates over in-memory signaling channels, opens the
  reliable channel on wildcard local UDP sockets, and transfers bytes without
  contacting an external network.
- Removed the pre-existing argument-less `reqwest::ClientBuilder::connector_layer`
  placeholder from `remote/client/src/http_client.rs`. It was not a functional
  layer and prevented the current direct client from compiling; Task 4 will add
  the actual P2P connector.
- Validation completed:

  ```text
  cargo fmt --all -- --check                                  PASS
  cargo clippy -p trz-gateway-common --all-targets -- -D warnings  PASS
  cargo test -p trz-gateway-common                            PASS (54 tests)
  cargo check -p trz-gateway-client -p trz-gateway-server     PASS
  bazel test --test_output=errors //remote/common:common-test PASS
  ```

## Task 2: Signaling node

1. Add `remote/server/src/server/p2p/signaling.rs` (and subordinate protocol/state
   files as needed), store its registry in `Server`, and register the two routes
   in `make_app`.
2. On `/p2p/register/{server_name}`, atomically install a new registration,
   cancel and displace any previous registration under that name, notify waiting
   clients, and relay session starts, client offers, and client ICE candidates to
   the registered server. Relay answers, server candidates, cancellation, and
   failures back to the matching client. Guard removal by registration generation
   so stale cleanup cannot remove the current registration.
3. On `/p2p/connect/{server_name}`, wait up to 30 seconds for a registration,
   allocate the session against the registration generation found, attach it to
   that registration, and enforce the separate handshake timeout. Remove the
   waiter promptly if the HTTP request is cancelled and return 404 if the wait
   expires.
4. Do not interpret SDP beyond size/type validation and do not become a fallback
   data relay. A connection that cannot find a viable ICE pair should fail with a
   useful error that points to TURN configuration.
5. Tie all registry and relay tasks to `Server::shutdown` and add focused Axum
   tests for replacement cancelling the old registration and sessions, stale
   cleanup preserving the replacement, connect-before-register wakeup, the
   30-second offline timeout, cancelled-waiter cleanup, correct session routing,
   malformed messages, handshake timeout, disconnect cleanup, and shutdown. Use
   Tokio's paused clock for the 30-second behavior rather than slowing the suite.

## Task 3: NATed server role

1. Add optional P2P registration configuration to the gateway/server config:
   signaling URL, this server's `ClientName`, ICE server URLs, retry strategy,
   handshake timeout, and registration authorization material. Default to
   disabled so existing servers behave exactly as today.
2. Start a registration task from `Server::run`. It connects outward to the
   signaling endpoint over WSS, retries with the existing bounded backoff pattern,
   and creates an answering `RTCPeerConnection` for every relayed offer. One failed
   session must not drop the server's registration or other sessions.
3. When a reliable data channel opens, pass its `DataChannelIo` to a new
   `Server::serve_p2p_connection` helper. Refactor the current endpoint setup so
   both TCP listeners and P2P connections use the same router from `make_app`.
4. Apply the gateway's existing TLS server configuration to the P2P byte stream,
   then use Hyper/Hyper-util's auto HTTP connection builder to serve the cloned
   Axum router with HTTP/1.1 and HTTP/2 enabled. Preserve request tracing and the
   same header/read/keepalive timeouts as practical for a non-socket transport.
5. Stop peer connections and the registration loop when the gateway handle shuts
   down; bound concurrent peer sessions so public signaling cannot exhaust the
   NATed server.

Serving TLS again inside WebRTC is intentional. WebRTC DTLS secures the peer link,
while the existing Terrazzo TLS certificate authenticates the requested server
name and preserves all current `/remote/certificate` security behavior.

## Task 4: Client connector and configuration

1. Extend `ClientConfig` with a backward-compatible transport selection, for
   example `Direct` (the default) or `WebRtc { signaling_url, server_name,
   ice_servers }`. Keep the target HTTPS authority/SNI separate from the signaling
   URL: it identifies the server certificate and supplies the HTTP authority, but
   it is never DNS-resolved or dialed in P2P mode.
2. Add a client-side signaling session that opens
   `/p2p/connect/{server_name}`, creates the offer and reliable data channel,
   trickles candidates in both directions, and returns `DataChannelIo` only when
   the channel is open. Include signaling, ICE, and total-connect timeouts and
   close the peer connection on cancellation.
3. Replace the placeholder in `remote/client/src/http_client.rs` with a real
   connector layer for P2P mode. The connector must supply a fresh WebRTC byte
   stream for each connection request while leaving reqwest's normal TLS and HTTP
   stack above it, so certificate validation, SNI override, pooling, and HTTP/2
   continue to work. Direct mode builds the current client unchanged.
4. Factor transport selection so `remote/client/src/client/connect.rs` can use the
   same connector abstraction rather than hard-coding TCP/WebSocket setup. Keep
   the existing WebSocket tunnel as the direct fallback and preserve its retry,
   health, shutdown, and gRPC-serving behavior.
5. Verify reqwest's `connector_layer` can replace the dialed stream before the TLS
   layer with the pinned reqwest version. If its layer boundary is too high, use a
   small Hyper client connector behind the same internal abstraction instead of
   weakening or duplicating TLS verification.

## Task 5: End-to-end certificate test

Add a test in `remote/client/src/tests.rs` with a dedicated P2P fixture:

1. Start a signaling `trz_gateway_server` on a dynamically allocated localhost
   port with an allow-all test registration policy.
2. Start a second gateway server configured with its own dynamically allocated TCP
   port, a globally unique test `ClientName`, P2P registration to the signaling
   server, and `stun:stun.l.google.com:19302`.
3. Wait until registration is visible through the signaling API. Do not pass the
   second server's socket address or port to the client fixture.
4. Configure the client with the signaling endpoint, target `ClientName`, target
   TLS authority/SNI, target trusted root, Google STUN, and the target server's
   current auth code. Call the existing `make_client_certificate` path; this must
   exercise the connector in `http_client.rs` and request
   `/remote/certificate` through WebRTC.
5. Parse the returned certificate and assert that it is signed by the target
   server's configured root and contains the expected client identity. Also assert
   signaling observed a completed session and transferred no application data.
6. Use strict per-phase and overall timeouts and shut down client peers, target
   server, and signaling server on every exit path. Because this explicitly uses
   Google's STUN service, mark the test with the repository's network-test tag or
   otherwise keep it out of hermetic/offline test groups while still running it in
   the network-enabled integration suite.

Add negative integration coverage for an unknown server name and a target TLS
name/root mismatch. The latter proves that successful WebRTC negotiation does not
bypass end-to-end server authentication.

## Validation

Run the smallest relevant checks after each task, then the combined paths:

```sh
cargo fmt --all -- --check
cargo test -p trz-gateway-common
cargo test -p trz-gateway-server
cargo test -p trz-gateway-client
RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features
bazel build //remote/...
bazel test --test_output=errors --verbose_failures //remote/...
bazel run //bazel:buildifier_check
```

Run the Google-STUN test separately in an environment with outbound UDP/DNS and
repeat it enough times to expose cleanup races. Unit and signaling tests remain
hermetic and should provide most failure diagnostics when that external test is
unavailable.

## Suggested review sequence

1. Shared protocol, `DataChannelIo`, and peer builder with unit tests.
2. Signaling registry and `/p2p/...` routes with local relay tests.
3. NATed server registration and serving the existing app over WebRTC.
4. Reqwest connector, client transport selection, and certificate test.
5. Terminal configuration wiring, hardening/limits, TURN deployment options, and
   operational documentation.

Do not remove the current socket or WebSocket paths during this work. Each stage
should keep direct mode green and land with tests for its lifecycle and cleanup.
