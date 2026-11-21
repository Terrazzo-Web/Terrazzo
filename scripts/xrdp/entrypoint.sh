#!/bin/bash
# Run this through the container image's ENTRYPOINT, rather than directly on the host.
# For build, recreate, shell, delete and other host-side operations, use ./manage.sh --help.
#
# On the Gentoo VM, from the repository root, build the image:
#   sudo podman build --format docker -t localhost/ubuntu-xrdp:26.04 containers/xrdp
#
# Create or update the password secret using hidden, confirmed input; the desktop username is richard:
#   ./manage.sh password
# If the container already exists, apply the updated secret with ./manage.sh recreate (ends desktop sessions).
#
# Start the container, publishing RDP on all host IPv4 interfaces and preserving the home directory:
#   sudo podman run -d --name ubuntu-xrdp \
#     --hostname ubuntu-container \
#     --secret rdp-password \
#     --shm-size=256m \
#     -p 0.0.0.0:13389:3389 \
#     -v /home/richard/xrdp:/home/richard \
#     localhost/ubuntu-xrdp:26.04
#
# Check startup and listener health:
#   sudo podman logs ubuntu-xrdp
#   sudo podman healthcheck run ubuntu-xrdp
#
# Allow inbound TCP 13389 in the Azure NSG and host firewall for the intended client addresses.
# An SSH tunnel remains optional if direct access is not available.
# The generated RDP certificate is self-signed; verify it before accepting a client certificate prompt.
#
# Stop and restart the existing container without recreating it:
#   sudo podman stop ubuntu-xrdp
#   sudo podman start ubuntu-xrdp
# Automatic startup when the Gentoo host boots requires separate OpenRC configuration.
# No --privileged flag, host networking, or systemd is required.

set -Eeuo pipefail

if [[ $(id -u) != 0 ]]; then
    echo 'The container entrypoint must run as container root; desktop login uses richard.' >&2
    exit 1
fi

if [[ ! -r "$RDP_PASSWORD_FILE" ]]; then
    echo "Mount a password file at $RDP_PASSWORD_FILE (for example, a Podman secret)." >&2
    exit 1
fi

password=$(cat "$RDP_PASSWORD_FILE")
if [[ -z "$password" || "$password" == *$'\n'* || "$password" == *$'\r'* ]]; then
    echo 'The RDP password must be a nonempty single line.' >&2
    exit 1
fi
printf 'richard:%s\n' "$password" | chpasswd
unset password

install -d -m 755 /run/dbus
install -d -m 755 -o xrdp -g xrdp /run/xrdp
install -d -m 3777 -o xrdp -g xrdp /run/xrdp/sockdir
dbus-uuidgen --ensure=/etc/machine-id

# Generate a container-specific TLS identity rather than baking a private key into the image.
if [[ ! -s /etc/xrdp/key.pem || ! -s /etc/xrdp/cert.pem ]]; then
    openssl req -x509 -newkey rsa:3072 -sha256 -nodes -days 365 \
        -subj '/CN=ubuntu-xrdp' \
        -keyout /etc/xrdp/key.pem -out /etc/xrdp/cert.pem >/dev/null 2>&1
fi
chown root:xrdp /etc/xrdp/key.pem
chmod 640 /etc/xrdp/key.pem
chmod 644 /etc/xrdp/cert.pem

children=()
cleanup() {
    trap - EXIT TERM INT
    if (( ${#children[@]} )); then
        kill "${children[@]}" 2>/dev/null || true
        wait "${children[@]}" 2>/dev/null || true
    fi
}
trap cleanup EXIT
trap 'exit 0' TERM INT

dbus-daemon --system --nofork --nopidfile &
children+=("$!")
/usr/sbin/xrdp-sesman --nodaemon &
children+=("$!")
/usr/sbin/xrdp --nodaemon &
children+=("$!")

echo 'Ubuntu Xfce desktop ready for xrdp login as richard on port 3389.'
# Fail the container if any required service exits; tini reaps orphaned session processes.
set +e
wait -n "${children[@]}"
status=$?
set -e
echo "A required desktop service exited (status $status); stopping the container." >&2
if (( status == 0 )); then
    exit 1
fi
exit "$status"
