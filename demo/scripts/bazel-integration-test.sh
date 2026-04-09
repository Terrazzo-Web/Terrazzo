#!/usr/bin/env bash

set -euo pipefail

SERVER_BIN="${1:?Usage: $0 <path-to-demo-server> <playwright-root>}"
PLAYWRIGHT_ROOT="${2:?Usage: $0 <path-to-demo-server> <playwright-root>}"

SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"
PLAYWRIGHT_ROOT="${TEST_SRCDIR}/${TEST_WORKSPACE}/${PLAYWRIGHT_ROOT}"

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

cp demo/scripts/integration-test.spec.mjs integration-test.spec.mjs

for _ in $(seq 1 60); do
  if curl --silent --fail http://127.0.0.1:3000/ >/dev/null; then
    export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_ROOT}/ms-playwright"
    "${PLAYWRIGHT_ROOT}/node_modules/.bin/playwright" test \
      integration-test.spec.mjs
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on http://127.0.0.1:3000" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
