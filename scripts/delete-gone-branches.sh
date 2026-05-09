#!/usr/bin/env bash
set -euo pipefail

remote="${1:-origin}"

git fetch --prune "$remote"

current_branch="$(git branch --show-current)"

git for-each-ref --format='%(refname:short) %(upstream:track)' refs/heads |
  awk '$2 == "[gone]" { print $1 }' |
  while IFS= read -r branch; do
    if [[ "$branch" == "$current_branch" ]]; then
      printf 'Skipping current branch: %s\n' "$branch" >&2
      continue
    fi

    git branch -D "$branch"
  done
