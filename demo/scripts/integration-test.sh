#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SERVER_BIN="$ROOT_DIR/target/release/demo-server"
SERVER_LOG="${SERVER_LOG:-$(mktemp -t terrazzo-demo-server.XXXXXX.log)}"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
}

trap cleanup EXIT

if [[ ! -x "${SERVER_BIN}" ]]; then
  echo "Expected executable at ${SERVER_BIN}. Build it first with:" >&2
  echo "  cargo build --bin demo-server --features server,max_level_info --release" >&2
  exit 1
fi

"${SERVER_BIN}" >"${SERVER_LOG}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 60); do
  if curl --silent --fail http://127.0.0.1:3000/ >/dev/null; then
    npx playwright test "${ROOT_DIR}/demo/scripts/integration-test.spec.mjs"
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on http://127.0.0.1:3000" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
