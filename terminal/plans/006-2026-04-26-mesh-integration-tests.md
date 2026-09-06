# Mesh integration tests

The tests start a single Terrazzo server.

I want to create a new integration test that starts 2 servers and connects them to each other.


## Desired outcome

The goal is to have an as much as possible hermetic test that starts two terrazzo servers and connects them using the mesh feature. The end result is the client node gets a certificate allocated by the server node.

## Work to do

- First you run the first server and check that it is healthy. Configuration is insipred by terminal/tests/run-server.sh and terminal/tests/config-server.toml, except: the pidfile, private root CA, must be in temp folders. Terrazzo will create the root CA if necessary. Also there is no need for password in integration tests, this is already tested separately.
- Then you run the client. The config is inspired by terminal/tests/run-client.sh and terminal/tests/config-client.toml. Same, the pidfile should be in temp folder, the private_root_ca must be shared with the server. The gateway_url must be the server's port, which should be dynamically allocated as currently in integration tests. The client certificate will be stored in a temp folder as well, along with pidfile and root CA.
- When the client starts, it should print a log "ailed to load Client Certificate: [Make] [GetCertificate] [HttpStatus] Gateway returned 403 Forbidden: [InvalidAuthCode] AuthCode is invalid". Kill the client. Look at the server, it should print a log Invalid auth code. Got '' expected '$AUTH_CODE' with the expected auth code GUID. Parse it, and restart the client with this auth code (add --auth-code command line when starting the client). Now start the client and verify it successfully acquires a certificate. Assert the subject name of the client certificate (it should contain the mesh.client_name of the client's configuration)

## Notes

This should be implemented as a Rust test. Try to use integration tests as usually done in rust (i.e. terminal/tests/mesh_integration_test.rs), else as a normal unit test. The test should run with bazel. You need to create a new bazel rust test target that depends on //terminal:server so it can run the terrazzo server.

You must first check that you have all the tools to do your work so I am not prompted all the time.

Then you need to update the plan with documentation of the test harness.

## Test harness

Implemented as `//terminal:mesh-integration-test` in `terminal/tests/mesh_integration_test.rs`.
The Bazel target is a `rust_test` that receives the built `:server-debug` binary as its first
argument and launches it directly, matching the existing Playwright wrapper's `CARGO_MANIFEST_DIR`
setup for Bazel-built terminal servers.

Added `//terminal/terrazzo-terminal-tests:terrazzo-terminal-tests`, a small binary harness that
starts a gateway node and a mesh client node. Its parameters are:

```text
terrazzo-terminal-tests <terrazzo-terminal-server-bin> <gateway-port> <gateway-set-current-endpoint> [-- <server-args>...]
```

The harness writes temp configs, starts the supplied Terrazzo Terminal server binary as a gateway
with the requested port and gateway endpoint file, then starts a client node configured with the
gateway URL reported by the gateway. It performs the same auth-code bootstrap as the current Rust
test: start a client with an invalid auth code, parse the expected auth code from the gateway log,
restart the client with `--auth-code`, wait for the client certificate, then keep both nodes running
until one exits or the harness is killed. The current `mesh_client_gets_certificate_from_gateway`
test is intentionally left launching the server directly until the later refactor.

The test creates a deterministic directory under `TEST_TMPDIR`, writes all configs, pidfiles,
endpoint files, logs, the shared private root CA, and the client certificate under that directory,
and starts:

- one gateway server with `port = 0` and `--set-current-endpoint`;
- one mesh client without `--auth-code`, which is expected to log the 403 certificate load failure;
- one mesh client restarted with the auth code parsed from the gateway log.

Readiness is checked by waiting for the dynamically allocated endpoint file and a TCP connection to
the reported endpoint. The gateway log is polled for `Invalid auth code. Got '' expected '...'`, and
the parsed auth code is passed to the second client via `--auth-code`. Success is asserted by waiting
for the generated client certificate and verifying its X.509 subject common name contains the
configured `mesh.client_name`.

Run it with:

```sh
bazel test //terminal:mesh-integration-test
```
