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

mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
