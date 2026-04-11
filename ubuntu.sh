#!/usr/bin/env bash

set -euo pipefail
cd "$(dirname "$0")"

WORKSPACE_DIR="$(pwd)"
WORKSPACE_NAME="$(basename "$WORKSPACE_DIR")"

IMAGE_NAME="${DEVBOX_IMAGE_NAME:-dev-box}"
CONTAINER_FILE="${DEVBOX_CONTAINER_FILE:-$(pwd)/Dockerfile}"
CONTAINER_NAME="${DEVBOX_CONTAINER_NAME:-dev-box-$WORKSPACE_NAME}"
CONTAINER_IDLE_TIMEOUT_SECONDS="${DEVBOX_CONTAINER_TTL:-1200}"

TMPDIR="${TMPDIR:-/tmp}"

usage() {
  cat <<'EOF'
Usage: ./ubuntu.sh <command> [args...]

Runs the given command inside a persistent Ubuntu container.

Useful example:
- List active commands: ./ubuntu.sh sh -lc 'tail -v /tmp/DevBox/active/*'

Environment overrides:
- DEVBOX_IMAGE_NAME: image tag to run (default: 'DevBox')
- DEVBOX_CONTAINER_FILE: Dockerfile path to build from if image is missing
- DEVBOX_CONTAINER_NAME: container name to reuse
- DEVBOX_CONTAINER_TTL: idle shutdown timeout in seconds (default: 1200, i.e. 20 minutes)
EOF
}

if [[ $# -eq 0 ]]; then
  usage
  exit 1
fi

# TODO: if the $CONTAINER_FILE content chained, rebuild the image and start a new container.
# if ! podman image exists "$IMAGE_NAME"; then
  if [[ ! -f "$CONTAINER_FILE" ]]; then
    echo "Image '$IMAGE_NAME' does not exist and no Dockerfile was found at '$CONTAINER_FILE'." >&2
    exit 1
  fi

  echo "Building image '$IMAGE_NAME' from '$CONTAINER_FILE'..." >&2
  podman build -t "$IMAGE_NAME" -f "$CONTAINER_FILE" "$(dirname "$CONTAINER_FILE")"
# fi

if ! podman container exists "$CONTAINER_NAME"; then
  podman run -d \
    --name "$CONTAINER_NAME" \
    -v "$WORKSPACE_DIR:/workspace:Z" \
    -e "XDG_CACHE_HOME=/cache/xdg" \
    -e "BAZELISK_HOME=/cache/bazelisk" \
    -w /workspace \
    "$IMAGE_NAME" \
    bash -lc "
      set -euo pipefail
      idle_timeout='$CONTAINER_IDLE_TIMEOUT_SECONDS'

      mkdir -p $TMPDIR/DevBox/active
      touch $TMPDIR/DevBox/last-exec

      while true; do
        sleep 5

        if find $TMPDIR/DevBox/active -mindepth 1 -maxdepth 1 -print -quit | grep -q .; then
          continue
        fi

        last_exec=\$(stat -c %Y $TMPDIR/DevBox/last-exec 2>/dev/null || echo 0)
        now=\$(date +%s)

        if (( now - last_exec < idle_timeout )); then
          continue
        fi

        exit 0
      done
    " >/dev/null
fi

if [[ "$(podman inspect -f '{{.State.Running}}' "$CONTAINER_NAME")" != "true" ]]; then
  podman start "$CONTAINER_NAME" >/dev/null
fi

exec_args=()
in_foreground_tty=false
if [[ -t 0 ]]; then
  shell_pgid="$(ps -o pgid= -p $$ | tr -d ' ')"
  tty_pgid="$(ps -o tpgid= -p $$ | tr -d ' ')"
  if [[ -n "$shell_pgid" && "$shell_pgid" == "$tty_pgid" ]]; then
    in_foreground_tty=true
  fi
fi

if [[ "$in_foreground_tty" == "true" && -t 0 && -t 1 ]]; then
  exec_args+=(-it)
elif [[ "$in_foreground_tty" == "true" && -t 0 ]]; then
  exec_args+=(-i)
fi

exec podman exec "${exec_args[@]}" \
  --workdir /workspace \
  "$CONTAINER_NAME" \
  bash -lc "
    set -euo pipefail

    mkdir -p $TMPDIR/DevBox/active
    touch $TMPDIR/DevBox/last-exec

    marker_file=\$(mktemp $TMPDIR/DevBox/active/exec.XXXXXX)
    printf '%q ' \"\$@\" >\"\$marker_file\"
    printf '\n' >>\"\$marker_file\"

    cleanup() {
      rm -f \"\$marker_file\"
      touch $TMPDIR/DevBox/last-exec
    }

    trap cleanup EXIT

    \"\$@\"
  " bash "$@"
