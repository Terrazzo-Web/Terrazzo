#!/usr/bin/env bash

set -euo pipefail

SERVER_BIN="${1:?Usage: $0 <path-to-demo-server>}"

SERVER_BIN="${TEST_SRCDIR}/${TEST_WORKSPACE}/${SERVER_BIN}"

./demo/scripts/integration-test.sh "${SERVER_BIN}" 
