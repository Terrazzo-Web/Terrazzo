#!/usr/bin/env bash

set -euo pipefail

SERVER_BIN="${1:?Usage: $0 <path-to-demo-server>}"
PLAYWRIGHT_ROOT="${2:?Usage: $0 <path-to-demo-server> <playwright-root>}"

SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"
PLAYWRIGHT_ROOT="${TEST_SRCDIR}/${TEST_WORKSPACE}/${PLAYWRIGHT_ROOT}"

./demo/scripts/integration-test.sh "${SERVER_BIN}" "${PLAYWRIGHT_ROOT}"
