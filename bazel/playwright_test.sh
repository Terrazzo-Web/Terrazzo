#!/usr/bin/env bash

set -euo pipefail

USAGE="Usage: $0 <path-to-server-or-launcher> <path-to-target-server-or-> <playwright-root> <node-bin> <npx-bin> <test-spec>"
SERVER_BIN="${1:?${USAGE}}"
TARGET_SERVER="${2:?${USAGE}}"
PLAYWRIGHT_ROOT="${3:?${USAGE}}"
NODE_BIN="${4:?${USAGE}}"
NPX_BIN="${5:?${USAGE}}"
TEST_SPEC="${6:?${USAGE}}"

if [[ "${TARGET_SERVER}" != "-" ]]; then
  TARGET_SERVER="${TEST_SRCDIR}/${TEST_WORKSPACE}/${TARGET_SERVER}"
fi
SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"
NODE_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${NODE_BIN}"
NPX_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${NPX_BIN}"
TEST_SPEC="${TEST_SRCDIR}/${TEST_WORKSPACE}/${TEST_SPEC}"
if [[ "${TARGET_SERVER}" == "-" ]]; then
  SERVER_MANIFEST_BIN="${SERVER_BIN}"
else
  SERVER_MANIFEST_BIN="${TARGET_SERVER}"
fi
CARGO_MANIFEST_DIR="$(dirname "$(realpath "${SERVER_MANIFEST_BIN}")")/cargo_root/$(basename "${SERVER_MANIFEST_BIN}")"

TMPDIR_ROOT="${TMPDIR:-/tmp}"
TEST_TMPDIR="${TEST_TMPDIR:-$(mktemp -d "${TMPDIR_ROOT%/}/terrazzo-playwright.XXXXXX")}"
SERVER_LOG="${SERVER_LOG:-${TEST_TMPDIR%/}/server.log}"
SERVER_ENDPOINT_FILE="${SERVER_ENDPOINT_FILE:-${TEST_TMPDIR%/}/server-endpoint}"
TERRAZZO_CONFIG_FILE="${TERRAZZO_CONFIG_FILE:-${TEST_TMPDIR%/}/terrazzo-integration-test/gateway/config.toml}"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    pkill -TERM -P "${SERVER_PID}" 2>/dev/null || true
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  rm -f "${SERVER_LOG}" "${SERVER_ENDPOINT_FILE}"
}

trap cleanup EXIT

export TEST_TMPDIR

SERVER_ARGS=(
  --port 0
  --set-current-endpoint "${SERVER_ENDPOINT_FILE}"
)
if [[ "${TARGET_SERVER}" != "-" ]]; then
  SERVER_ARGS=(--server-bin "${TARGET_SERVER}" "${SERVER_ARGS[@]}")
  if "${SERVER_BIN}" --help 2>/dev/null | grep -Fq -- "--server-manifest-dir"; then
    SERVER_ARGS+=(--server-manifest-dir "${CARGO_MANIFEST_DIR}")
  fi
fi
CARGO_MANIFEST_DIR="${CARGO_MANIFEST_DIR}" \
RUST_BACKTRACE=1 \
"${SERVER_BIN}" \
    "${SERVER_ARGS[@]}" \
  > "${SERVER_LOG}" 2>&1 &

SERVER_PID="$!"

for _ in $(seq 1 30); do
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
    export TERRAZZO_SERVER_BIN="${SERVER_MANIFEST_BIN}"
    export TERRAZZO_CONFIG_FILE
    ln -s "${PLAYWRIGHT_ROOT}/node_modules" "node_modules"
    ln -s "${PLAYWRIGHT_ROOT}/package.json" "package.json"
    ln -s "${PLAYWRIGHT_ROOT}/package-lock.json" "package-lock.json"
    cp "${TEST_SPEC}" "$(basename "${TEST_SPEC}")"
    BAZEL=1 BASE_URL="${SERVER_URL}" "${NPX_BIN}" playwright test "$(basename "${TEST_SPEC}")" \
      || (cat "${SERVER_LOG}" >&2 ; exit 1)
    exit 0
  fi
  sleep 1
done

echo "Timed out waiting for demo server on ${SERVER_URL:-http://127.0.0.1:<unknown-port>/}" >&2
echo "Server log:" >&2
cat "${SERVER_LOG}" >&2
exit 1
