#!/usr/bin/env bash
set -euo pipefail

output_dir="$(realpath "$1")"
package_json="$(realpath "$2")"
package_lock="$(realpath "$3")"

mkdir -p "$output_dir"
cp "$package_json" "$output_dir/package.json"
cp "$package_lock" "$output_dir/package-lock.json"

# HOME and TMPDIR must match values set at test execution time.
export HOME="$output_dir/home"
export TMPDIR="$output_dir/tmp"
mkdir -p "$HOME" "$TMPDIR"

cd "$output_dir"
npm ci
./node_modules/.bin/playwright install chromium
