#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
  echo "BUILD_WORKSPACE_DIRECTORY is not set. Run this target with 'bazel run'." >&2
  exit 1
fi

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <src> <dest>" >&2
  exit 1
fi

src="$1"
dest="${BUILD_WORKSPACE_DIRECTORY}/$2"

normalize() {
  tr -d ' \t\n\r'
}

src_normalized="$(normalize < "$src")"
if [[ -f "$dest" ]]; then
  dest_normalized="$(normalize < "$dest")"
else
  dest_normalized=""
fi

if [[ "$src_normalized" == "$dest_normalized" ]]; then
  exit 0
fi

mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
