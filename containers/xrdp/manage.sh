#!/usr/bin/env bash
# Manage the rootful desktop container; automatically use sudo when necessary.
set -euo pipefail

directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
name=ubuntu-xrdp
hostname=ubuntu-container
image=localhost/ubuntu-xrdp:26.04
publish=0.0.0.0:13389:3389
home_directory=/home/richard/xrdp
secret=rdp-password

usage() {
    cat <<'EOF'
Usage: ./manage.sh [OPTIONS] COMMAND [ARGS...]

Manage the Ubuntu Xfce/xrdp container using rootful Podman.
Run as your normal user; the script invokes sudo when needed.

Commands:
  build                   Build the image from this script's directory.
  password [--file FILE]   Create/replace the password secret; prompt twice if no file.
  create, recreate        Remove the existing container, then create and start it.
  shell [COMMAND [ARG...]] Open a root shell (default: bash), or execute a command.
  start                   Start the existing container.
  stop                    Stop the container gracefully.
  status                  Show its state, health and hostname.
  logs [PODMAN_LOG_ARGS]   Show logs; use "logs -f" to follow them.
  delete                  Remove the container if it exists, stopping it if needed.
  help                    Show this help.

Options (before COMMAND):
  --name NAME             Container name (default: ubuntu-xrdp).
  --hostname HOSTNAME     Container hostname (default: ubuntu-container).
  --image IMAGE           Image to build/run (default: localhost/ubuntu-xrdp:26.04).
  --publish MAPPING       Port mapping (default: 0.0.0.0:13389:3389).
  --home-dir DIRECTORY    Host directory mounted at /home/richard (default: /home/richard/xrdp).
  --secret NAME           Password secret to use or update (default: rdp-password).
  -h, --help              Show this help without invoking Podman or sudo.

Examples:
  ./manage.sh build
  ./manage.sh password
  ./manage.sh recreate
  ./manage.sh shell
  ./manage.sh shell sudo -n id
  ./manage.sh logs -f
  ./manage.sh delete

Create/recreate checks that the image and password secret exist BEFORE deleting
anything. Create the secret with "./manage.sh password" first.
The container's desktop username is richard; its shell command runs as root.

Password input is hidden and confirmed. For automation, use "password --file FILE"
or "password --file -" to read stdin; no password is accepted as a command argument.
Updating a secret does NOT change an existing container's password. Run recreate
afterward to apply it; restarting the existing container is not sufficient.

Delete/recreate preserves the host home directory, password secret and image.
The image's richard account uses UID/GID 1000, matching the Gentoo host account.
Recreation ends desktop sessions and discards changes outside /home/richard.
The default mapping listens on all host IPv4 interfaces. Connect RDP to the
Gentoo host's address on port 13389; Azure and host firewall rules still apply.
EOF
}

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

# Parse global options without requiring root for help or argument validation.
while (( $# )); do
    case "$1" in
        -h|--help|help) usage; exit 0 ;;
        --name|--hostname|--image|--publish|--home-dir|--secret)
            (( $# >= 2 )) && [[ -n "$2" && "$2" != -* ]] || fail "$1 requires a value"
            case "$1" in
                --name) name=$2 ;;
                --hostname) hostname=$2 ;;
                --image) image=$2 ;;
                --publish) publish=$2 ;;
                --home-dir) home_directory=$2 ;;
                --secret) secret=$2 ;;
            esac
            shift 2
            ;;
        --) shift; break ;;
        -*) fail "Unknown option: $1 (see --help)" ;;
        *) break ;;
    esac
done

if (( $# == 0 )); then
    usage
    exit 0
fi
command=$1
shift
case "$command" in
    build|create|recreate|start|stop|status|delete)
        (( $# == 0 )) || fail "$command does not accept arguments"
        ;;
    shell|logs) ;;
    password)
        if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
            usage
            exit 0
        fi
        (( $# == 0 )) || [[ $# == 2 && "$1" == --file && -n "$2" ]] \
            || fail 'Usage: password [--file FILE|-]'
        ;;
    *) fail "Unknown command: $command (see --help)" ;;
esac

# Always use the rootful image/container/secret store, matching the documented setup.
command -v podman >/dev/null || fail 'Podman is not installed'
podman=(podman)
if (( EUID != 0 )); then
    command -v sudo >/dev/null || fail 'Run as root or install sudo'
    podman=(sudo -- podman)
fi

case "$command" in
    password)
        # Never expose credentials through shell tracing, argv or environment variables.
        set +x
        if (( $# )); then
            if [[ "$2" == - ]]; then
                password=$(cat)
            else
                [[ -r "$2" ]] || fail "Cannot read password file: $2"
                password=$(cat -- "$2")
            fi
        else
            [[ -t 0 ]] || fail 'Interactive input required; use password --file FILE or --file -'
            # Authenticate sudo before prompting, so it cannot consume the secret's stdin.
            if (( EUID != 0 )); then
                sudo -v
            fi
            IFS= read -r -s -p 'New RDP password: ' password
            printf '\n' >&2
            IFS= read -r -s -p 'Confirm RDP password: ' confirmation
            printf '\n' >&2
            [[ "$password" == "$confirmation" ]] || fail 'Passwords do not match; secret unchanged'
            unset confirmation
        fi
        [[ -n "$password" && "$password" != *$'\n'* && "$password" != *$'\r'* ]] \
            || fail 'Password must be a nonempty single line; secret unchanged'
        # Use Podman's replacement API rather than explicitly deleting the working secret first.
        printf '%s' "$password" | "${podman[@]}" secret create --replace "$secret" - >/dev/null
        unset password
        printf 'Password secret "%s" updated.\n' "$secret"
        printf 'Recreate container "%s" with its usual options to apply it; this ends desktop sessions.\n' "$name"
        ;;
    build)
        "${podman[@]}" build --format docker -t "$image" "$directory"
        ;;
    create|recreate)
        # Do not interrupt an existing desktop for an absent image or password secret.
        "${podman[@]}" image inspect "$image" >/dev/null \
            || fail "Image $image is unavailable; run '$0 build' first"
        "${podman[@]}" secret inspect "$secret" >/dev/null \
            || fail "Password secret $secret is unavailable"
        # Require an existing absolute host directory; never create or chown a user's data implicitly.
        [[ "$home_directory" == /* && -d "$home_directory" ]] \
            || fail "Home directory must be an existing absolute path: $home_directory"
        "${podman[@]}" rm --force --ignore "$name"
        "${podman[@]}" run -d --name "$name" \
            --hostname "$hostname" \
            --secret "$secret,target=rdp-password" \
            --shm-size=256m \
            -p "$publish" \
            -v "$home_directory:/home/richard" \
            "$image"
        ;;
    shell)
        # Allocate a TTY for interactive use, but also allow commands in pipes/scripts.
        terminal=(-i)
        if [[ -t 0 && -t 1 ]]; then
            terminal+=(-t)
        fi
        if (( $# == 0 )); then
            set -- bash
        fi
        exec "${podman[@]}" exec "${terminal[@]}" --user root "$name" "$@"
        ;;
    start|stop)
        "${podman[@]}" "$command" "$name"
        ;;
    status)
        "${podman[@]}" inspect --format \
            'Name={{.Name}} State={{.State.Status}} Health={{.State.Health.Status}} Hostname={{.Config.Hostname}}' "$name"
        ;;
    logs)
        "${podman[@]}" logs "$@" "$name"
        ;;
    delete)
        # Remove only the container; the host home directory remains untouched.
        "${podman[@]}" rm --force --ignore "$name"
        ;;
esac
