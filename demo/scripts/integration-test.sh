#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SERVER_BIN_INPUT="${1:-}"
SERVER_BIN=""

PLAYWRIGHT_ROOT_INPUT="${2:-}"
PLAYWRIGHT_ROOT=""

SERVER_LOG="${SERVER_LOG:-$(mktemp --tmpdir terrazzo-demo-server.XXXXXX.log)}"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  rm -f "$SERVER_LOG"
}

trap cleanup EXIT

if [[ -z "${SERVER_BIN_INPUT}" ]]; then
  echo "Usage: $0 <path-to-demo-server>" >&2
  exit 1
fi

if [[ "${SERVER_BIN_INPUT}" = /* ]]; then
  SERVER_BIN="${SERVER_BIN_INPUT}"
else
  SERVER_BIN="$ROOT_DIR/${SERVER_BIN_INPUT}"
fi

if [[ -n "${PLAYWRIGHT_ROOT_INPUT}" ]]; then
  if [[ "${PLAYWRIGHT_ROOT_INPUT}" = /* ]]; then
    PLAYWRIGHT_ROOT="${PLAYWRIGHT_ROOT_INPUT}"
  else
    PLAYWRIGHT_ROOT="$ROOT_DIR/${PLAYWRIGHT_ROOT_INPUT}"
  fi
fi

if [[ ! -x "${SERVER_BIN}" ]]; then
  echo "Expected executable at ${SERVER_BIN}." >&2
  exit 1
fi

"${SERVER_BIN}" >"${SERVER_LOG}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 60); do
  if curl --silent --fail http://127.0.0.1:3000/ >/dev/null; then
    (
      echo "CURRENT DIR: $(pwd)"
      echo ""; echo ""

      ls -la
      echo ""; echo ""

      echo cp demo/scripts/integration-test.spec.mjs integration-test.spec.mjs
      cp demo/scripts/integration-test.spec.mjs integration-test.spec.mjs
      ls -la
      echo ""; echo ""

      if [[ -n "${PLAYWRIGHT_ROOT}" ]]; then
        export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_ROOT}/ms-playwright"
        "${PLAYWRIGHT_ROOT}/node_modules/.bin/playwright" test integration-test.spec.mjs
      else
        echo npm install
        npm install
        ls -la
        echo ""; echo ""

        echo npx playwright install --with-deps chromium
        npx playwright install --with-deps chromium
        ls -la
        echo ""; echo ""

        echo ls -la node_modules
        ls -la node_modules
        echo ""; echo ""

        npx playwright test integration-test.spec.mjs
      fi
    )
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on http://127.0.0.1:3000" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
