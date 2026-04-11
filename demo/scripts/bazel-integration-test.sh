#!/usr/bin/env bash

set -euo pipefail

SERVER_BIN="${1:?Usage: $0 <path-to-demo-server> <playwright-root> <node-bin> <npx-bin> <test-spec>}"
PLAYWRIGHT_ROOT="${2:?Usage: $0 <path-to-demo-server> <playwright-root> <node-bin> <npx-bin> <test-spec>}"
NODE_BIN="${3:?Usage: $0 <path-to-demo-server> <playwright-root> <node-bin> <npx-bin> <test-spec>}"
NPX_BIN="${4:?Usage: $0 <path-to-demo-server> <playwright-root> <node-bin> <npx-bin> <test-spec>}"
TEST_SPEC="${5:?Usage: $0 <path-to-demo-server> <playwright-root> <node-bin> <npx-bin> <test-spec>}"

SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"
NODE_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${NODE_BIN}"
NPX_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${NPX_BIN}"
TEST_SPEC="${TEST_SRCDIR}/${TEST_WORKSPACE}/${TEST_SPEC}"

SERVER_LOG="${SERVER_LOG:-$(mktemp --tmpdir terrazzo-demo-server.XXXXXX.log)}"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  rm -f "${SERVER_LOG}"
}

trap cleanup EXIT

# TODO the port should be randomized
"${SERVER_BIN}" --port 3000 > "${SERVER_LOG}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 60); do
  if curl --silent --fail http://127.0.0.1:3000/ >/dev/null; then
    export HOME="$PLAYWRIGHT_ROOT/home"
    export TMPDIR="$PLAYWRIGHT_ROOT/tmp"
    export PATH="$(dirname "${NODE_BIN}"):${PATH:-}"
    ln -s "${PLAYWRIGHT_ROOT}/node_modules" "node_modules"
    ln -s "${PLAYWRIGHT_ROOT}/package.json" "package.json"
    ln -s "${PLAYWRIGHT_ROOT}/package-lock.json" "package-lock.json"
    cp "${TEST_SPEC}" "$(basename "${TEST_SPEC}")"
    "${NPX_BIN}" playwright test "$(basename "${TEST_SPEC}")"
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on http://127.0.0.1:3000" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
