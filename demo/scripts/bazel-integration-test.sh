#!/usr/bin/env bash

set -euo pipefail

SERVER_BIN="${1:?Usage: $0 <path-to-demo-server> <playwright-root> <test-spec>}"
PLAYWRIGHT_ROOT="${2:?Usage: $0 <path-to-demo-server> <playwright-root> <test-spec>}"
TEST_SPEC="${3:?Usage: $0 <path-to-demo-server> <playwright-root> <test-spec>}"

SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"
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

"${SERVER_BIN}" >"${SERVER_LOG}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 60); do
  if curl --silent --fail http://127.0.0.1:3000/ >/dev/null; then
    export HOME="$PLAYWRIGHT_ROOT/home"
    export TMPDIR="$PLAYWRIGHT_ROOT/tmp"
    ln -s "${PLAYWRIGHT_ROOT}/node_modules" "node_modules"
    ln -s "${PLAYWRIGHT_ROOT}/package.json" "package.json"
    ln -s "${PLAYWRIGHT_ROOT}/package-lock.json" "package-lock.json"
    cp "${TEST_SPEC}" "$(basename "${TEST_SPEC}")"
    ./node_modules/.bin/playwright test "$(basename "${TEST_SPEC}")"
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on http://127.0.0.1:3000" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
