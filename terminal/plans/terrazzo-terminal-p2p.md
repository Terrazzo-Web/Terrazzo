# Terrazzo Terminal mesh over WebRTC

## Goal

Allow a `terrazzo-terminal` mesh client to reach a gateway through the existing
WebRTC signaling transport instead of dialing `mesh.gateway_url` directly. The
same configured target authority, SNI override, trusted root, certificate
enrollment, WebSocket tunnel, and gRPC services must remain in use above the
transport.

Add an end-to-end process test that starts terminal nodes in this topology:

```text
                                            outbound registration
  terminal gateway node  ----------------------------------------------+
       (P2P server)                                                       |
            ^                                                            v
            | reliable WebRTC data channel                    terminal signaling node
            | (inner target TLS + HTTP/WebSocket/gRPC)         /p2p/register + /p2p/connect
            |                                                            ^
  terminal client node --------------------------------------------------+
       (mesh client)                 signaling only
```

The mesh client must not receive or dial the gateway node's TCP endpoint. Both
WebRTC peers use `trz_gateway_common::p2p::GOOGLE_STUN`; the signaling node only
relays negotiation messages.

Direct mesh connections remain the default and all existing configurations and
integration tests must continue to work unchanged.

## Existing implementation and reusable pieces

- `terminal/src/backend/config/mesh.rs` currently requires a single
  `gateway_url`. It has no transport selection.
- `terminal/src/backend/agent.rs` implements `ClientConfig` for
  `AgentClientConfig` and `AgentTunnelConfig`, but neither implementation
  overrides `ClientConfig::transport()`. They therefore always use
  `ClientTransport::Direct`.
- `trz_gateway_client::client::config` already provides
  `ClientTransport::WebRtc(P2pClientConfig)`. Both initial certificate loading
  and the long-lived tunnel consume the same `ClientConfig`, so exposing this
  value from `AgentClientConfig` selects WebRTC for both paths without adding a
  terminal-specific connector.
- `TerminalBackendServer` implements `GatewayConfig`, but does not override
  `p2p_registration()`. A terminal gateway therefore cannot yet register its
  existing HTTP application with a signaling node.
- `remote/common/src/p2p/mod.rs` exports the shared `GOOGLE_STUN` constant. Do
  not copy the URL into terminal code or test fixtures.
- `remote/client/src/tests/p2p_certificate.rs` is the reference fixture for the
  network portion: it starts separate signaling and target gateways, waits for
  registration by opening and cancelling a signaling session, configures the
  client without the target socket address, requires a server-reflexive ICE
  candidate, verifies the issued certificate, uses bounded phase/overall
  timeouts, and shuts down all peers on every exit path.
- `terminal/integration` already owns process startup, dynamic endpoint files,
  temporary configuration and PKI paths, auth-code discovery, log polling, and
  child cleanup for a terminal gateway and mesh client. Extend these helpers
  rather than creating another independent process harness.

## Configuration design

### Mesh client transport

1. Add an optional WebRTC block to `MeshConfig`, represented by a terminal
   configuration DTO rather than serializing the gateway-client runtime type
   directly. Absence means direct transport. Presence means WebRTC and contains:

   - `signaling_url`: the public signaling node URL;
   - `server_name`: the globally unique routing name registered by the target
     gateway;
   - `ice_servers`: STUN/TURN entries with URL(s), optional username, and optional
     credential;
   - optional signaling, handshake, and overall connect durations.

   Preserve `gateway_url`, but document it as the target HTTPS authority in
   WebRTC mode. It is still used for the HTTP authority and target TLS identity;
   it must not be resolved or dialed by the P2P connector. `sni_override` keeps
   its current meaning.

2. Follow the existing `ConfigTypes` pattern so file fields may be omitted while
   runtime fields are fully resolved. During merge, default an omitted ICE list
   to one `IceServer` containing `GOOGLE_STUN`, and use the defaults from
   `P2pClientConfig::new` for omitted timeouts. Reject a partially specified
   WebRTC block instead of silently reverting to direct mode.

3. Round-trip the WebRTC block from runtime configuration in
   `Config::to_config_file`. Add merge/serialization tests for:

   - an old mesh configuration selecting direct mode;
   - a minimal WebRTC block receiving Google STUN and bounded timeout defaults;
   - explicit STUN/TURN credentials and timeouts surviving a round trip;
   - missing required signaling URL or server name producing a clear config
     error rather than disabling the mesh client.

4. Keep the first version file-config-only unless a command-line use case is
   required. If CLI flags are added, add them as a complete group and define
   their precedence over the config file alongside the current mesh flags; do
   not overload `--gateway-url` with the signaling URL.

An intended minimal configuration should read approximately as follows (final
TOML naming should follow the repository's existing kebab-case conventions):

```toml
[mesh]
client_name = "client-node"
gateway_url = "https://gateway.internal"
gateway_pki = "/path/to/gateway-root.cert"
client_certificate = "/path/to/client-certificate"

[mesh.web_rtc]
signaling_url = "https://signal.example"
server_name = "gateway-node"
```

### Gateway registration

5. Add a disabled-by-default terminal server configuration block that maps to
   `trz_gateway_server::server::gateway_config::p2p::P2pRegistrationConfig`. It
   needs the signaling URL, registered server name, ICE servers, retry strategy,
   handshake timeout, optional bearer authorization, and maximum sessions. Use a
   redacting wrapper for authorization so neither `Debug` nor config-related
   tracing exposes the bearer token.

6. Override `TerminalBackendServer::p2p_registration()` to snapshot the resolved
   configuration at gateway startup. This activates the already implemented
   remote-server registration and `serve_p2p_connection` path, which serves the
   same terminal Axum router with the gateway's existing inner TLS configuration.
   Document that this server-side setting is startup-scoped: changing it through
   the terminal config watcher does not restart the gateway's registration task.

7. Add server-config merge, round-trip, default, and redaction tests. Keep an
   absent block returning `None`, which preserves today's gateway behavior.

The gateway side is necessary even though the user-facing connection choice is
on `MeshConfig`: the target terminal process must advertise itself to signaling
before a WebRTC mesh client can connect to it.

## Agent wiring

8. Store a resolved `ClientTransport` in `AgentClientConfig`. Construct it once
   in `AgentTunnelConfig::new` from the merged mesh configuration, and delegate
   `transport()` from both `AgentClientConfig` and `AgentTunnelConfig`.

9. Ensure the exact same `AgentClientConfig` transport is used when calling
   `load_client_certificate` and later when `Client::new` establishes the tunnel.
   This is the core regression to guard: certificate enrollment must not use
   WebRTC and then accidentally return to a direct tunnel.

10. Keep debug output useful but safe: report `Direct` versus `WebRtc`, signaling
    authority, registered server name, and timeout values; rely on the redacted
    ICE/auth configuration for secrets. Never log candidate strings, SDP, TURN
    credentials, auth codes, or certificate private keys.

11. Add focused `agent.rs` tests with a small constructed mesh config to assert
    the `ClientConfig` view returns `Direct` for legacy configuration and the
    expected `ClientTransport::WebRtc` for P2P configuration. Configuration tests
    cover parsing/defaulting; the process test below covers the real connector.

## Terminal process integration test

12. Refactor `terminal/integration/src/server.rs` just enough to support three
    explicit roles without duplicating process management:

    - `Signaling`: a normal terminal gateway whose `/p2p/*` routes are used for
      signaling;
    - `GatewayP2p`: a terminal gateway with outbound P2P registration to the
      signaling endpoint and a unique registered name;
    - `ClientP2p`: a mesh client whose target authority/PKI belongs to
      `GatewayP2p`, while its WebRTC block contains only the signaling endpoint
      and registered name.

    Split the TOML helpers accordingly. Continue placing configs, endpoint files,
    logs, root CAs, client certificates, and pidfiles under the harness temp
    directory.

13. Give the signaling node and target gateway separate private roots. Wait for
    both dynamic listener endpoints and root certificates. Configure the target
    gateway's registration with the signaling endpoint and `GOOGLE_STUN`, then
    wait for registration using the `Hello`/`Start`/`Cancel` probe pattern from
    `p2p_certificate.rs`. Put this probe in a reusable async helper if sharing it
    with the remote-client test is practical; otherwise keep the terminal copy
    limited to typed `SignalMessage` handling rather than string matching.

14. Before starting the terminal processes, reuse the
    `assert_google_stun_candidate` approach from `p2p_certificate.rs`: build a
    `PeerConnectionBuilder` configured only with `GOOGLE_STUN`, create an offer,
    and require a `typ srflx` candidate within a bounded timeout. This proves the
    test actually has working STUN access instead of succeeding on host ICE.

15. Start the P2P mesh client first with an empty auth code. Its configuration
    must intentionally omit the target gateway's socket endpoint/port:

    - `mesh.gateway_url` contains only the target TLS authority (for the test,
      the hostname covered by the target certificate);
    - `mesh.gateway_pki` points to the target gateway root, not the signaling
      root;
    - `mesh.web_rtc.signaling_url` points to the signaling node;
    - `mesh.web_rtc.server_name` matches the target registration;
    - both peers use an ICE server built from `GOOGLE_STUN`.

    Assert the expected 403 certificate-enrollment failure, stop that client,
    parse the target gateway's current auth code, and restart the same P2P client
    with `--auth-code`, following the current direct integration flow.

16. On success, wait for the client certificate and parse it rather than treating
    file existence alone as sufficient. Assert its subject common name is the
    configured client name, its signature verifies against the target gateway's
    root, and it does not verify against the signaling node's root. Also wait for
    a client log emitted after the long-lived tunnel is established so the test
    proves both certificate HTTP and the mesh tunnel traversed WebRTC.

17. Add a guard that makes accidental direct dialing observable. The client must
    never be given the target endpoint, and its `gateway_url` should use a target
    authority that is valid for TLS but has no usable target TCP port. The test
    should fail if `ClientConfig::transport()` regresses to `Direct` rather than
    falling back and passing locally.

18. Add strict timeouts around STUN discovery, registration visibility,
    certificate enrollment, and tunnel establishment. Extend the existing RAII
    child cleanup so client, target gateway, and signaling node are stopped on
    every error path, and close the standalone STUN peer explicitly.

19. Put this in a dedicated network-enabled integration target instead of adding
    Google STUN to every Playwright run. Reuse the `terminal/integration` library
    and terminal server binary data dependency. Tag the Bazel target as requiring
    network access (and keep it out of offline/hermetic wildcard groups if that
    is the repository convention); for Cargo, mark the Google-STUN test ignored
    and document the exact `--ignored` invocation. Do not treat a missing
    server-reflexive candidate as a skip in the network target: it must be a
    failure with a clear outbound DNS/UDP diagnostic.

20. Preserve the current direct two-node integration path as a separate test or
    harness mode. This provides regression coverage for backward compatibility
    and prevents the P2P test from replacing validation of direct mesh transport.

## Suggested implementation sequence

1. Add terminal config DTOs, merge/default/round-trip behavior, and tests for the
   mesh-client WebRTC block.
2. Wire `ClientTransport` through `AgentClientConfig` and verify certificate and
   tunnel users share it.
3. Add terminal gateway registration configuration and the
   `GatewayConfig::p2p_registration()` override.
4. Refactor the integration harness into signaling, P2P gateway, and P2P client
   roles while retaining its direct mode.
5. Add the Google-STUN process test, certificate/root assertions, tunnel-ready
   assertion, Bazel target/tags, and documented Cargo command.

## Validation

Run focused checks while implementing:

```sh
cargo fmt --all -- --check
cargo test -p terrazzo-terminal backend::config
cargo test -p terrazzo-terminal backend::agent
cargo test -p terrazzo-integration-tests
cargo clippy -p terrazzo-terminal --all-targets --all-features -- -D warnings
bazel test --test_output=errors //terminal:terminal-test
bazel test --test_output=errors //terminal/integration:integration-test
```

Use the actual target names introduced by the BUILD changes if they differ from
the placeholders above. Then run the dedicated network test explicitly in an
environment with outbound DNS and UDP access, repeat it enough times to expose
registration/cleanup races, and finally run the repository-level validation
required by `terminal/AGENTS.md`:

```sh
RUSTFLAGS="-A unused-crate-dependencies" cargo test --workspace --all-features
bazel build //...
bazel test --test_output=errors --verbose_failures //...
bazel run //bazel:buildifier_check
```

## Acceptance criteria

- Existing mesh TOML without WebRTC settings still selects direct transport.
- A WebRTC mesh configuration uses the signaling URL only for signaling and uses
  `gateway_url` only as the target HTTP/TLS authority.
- Certificate enrollment and the long-lived terminal tunnel both use
  `ClientTransport::WebRtc`.
- A terminal gateway can opt into outbound P2P registration without changing
  the behavior of unconfigured gateways.
- The dedicated test proves Google STUN produced a server-reflexive candidate,
  the client was never given the target TCP endpoint, and the two terminal nodes
  completed certificate enrollment and tunnel setup through WebRTC.
- The issued client certificate is tied to the target gateway's PKI and expected
  client name, not the signaling node.
- P2P processes, peer connections, and temporary files are cleaned up on success,
  failure, and timeout.
