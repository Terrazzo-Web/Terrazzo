#!/usr/bin/env bash
set -euo pipefail

output_dir="$(realpath "$1")"
package_json="$2"
package_lock="$3"
node_bin="$(realpath "$4")"
npm_bin="$(realpath "$5")"

mkdir -p "$output_dir"
cp "$package_json" "$output_dir/package.json"
cp "$package_lock" "$output_dir/package-lock.json"

# HOME and TMPDIR must match values set at test execution time.
export HOME="$output_dir/home"
export TMPDIR="$output_dir/tmp"
mkdir -p "$HOME" "$TMPDIR"

export PATH="$(dirname "$node_bin")${PATH:+:$PATH}"

cd "$output_dir"
"$npm_bin" ci
./node_modules/.bin/playwright install chromium
