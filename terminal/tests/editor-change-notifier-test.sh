#!/usr/bin/env bash
set -euo pipefail
"${TEST_SRCDIR}/${TEST_WORKSPACE}/$1" --test "${TEST_SRCDIR}/${TEST_WORKSPACE}/terminal/tests/editor-change-notifier.test.mjs"
