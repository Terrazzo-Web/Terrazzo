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
SERVER_ENDPOINT_FILE="${SERVER_ENDPOINT_FILE:-$(mktemp --tmpdir terrazzo-demo-server-endpoint.XXXXXX)}"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  rm -f "${SERVER_LOG}" "${SERVER_ENDPOINT_FILE}"
}

trap cleanup EXIT

"${SERVER_BIN}" --port 0 --set_current_endpoint "${SERVER_ENDPOINT_FILE}" > "${SERVER_LOG}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 5); do
  if [[ -s "${SERVER_ENDPOINT_FILE}" ]]; then
    SERVER_ENDPOINT="$(<"${SERVER_ENDPOINT_FILE}")"
    SERVER_URL="http://${SERVER_ENDPOINT}"
  else
    SERVER_URL=""
  fi

  if [[ -n "${SERVER_URL}" ]] && curl --silent --fail "${SERVER_URL}" >/dev/null; then
    export HOME="$PLAYWRIGHT_ROOT/home"
    export TMPDIR="$PLAYWRIGHT_ROOT/tmp"
    export PATH="$(dirname "${NODE_BIN}"):${PATH:-}"
    ln -s "${PLAYWRIGHT_ROOT}/node_modules" "node_modules"
    ln -s "${PLAYWRIGHT_ROOT}/package.json" "package.json"
    ln -s "${PLAYWRIGHT_ROOT}/package-lock.json" "package-lock.json"
    cp "${TEST_SPEC}" "$(basename "${TEST_SPEC}")"
    BASE_URL="${SERVER_URL}" "${NPX_BIN}" playwright test "$(basename "${TEST_SPEC}")" \
      || (cat "${SERVER_LOG}" >&2 ; exit 1)
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on ${SERVER_URL:-http://127.0.0.1:<unknown-port>/}" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
