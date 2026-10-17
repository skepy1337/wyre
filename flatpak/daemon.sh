#!/bin/sh
set -eu

UNIT=wyre-daemon.service
bin_dir=$(dirname "$0")
installation=${0%/app/dev.skepy.wyre/*}
# the "active" link the service uses
active=${0%/files/bin/daemon.sh}

unsafe() {
    for file in "$bin_dir/daemon.sh" "$bin_dir/flatbox"; do
        path=$(readlink -e "$file") || { echo "$file can't be found"; return; }
        while :; do
            # owned by root, and not writable by its group or anyone else
            if [ "$(stat -c %u "$path")" != 0 ] ||
                [ $((0$(stat -c %a "$path") & 022)) != 0 ]; then
                echo "$path can be changed by someone other than root"
                return
            fi
            [ "$path" != / ] || break
            path=$(dirname "$path")
        done
    done
}

problem=$(unsafe)
if [ -n "$problem" ]; then
    echo "$problem" >&2
    exit 1
fi
[ "${1:-}" != --check ] || exit 0

# flatbox only shares /run/wyre, where the daemon's socket goes, if it exists
mkdir -p /run/wyre

# restarts the service once an update repoints the "active"
# link, or disables it once Wyre is uninstalled
if [ -n "${INVOCATION_ID:-}" ] && started=$(readlink "$active"); then
    while sleep 60; do
        now=$(readlink "$active" || true)
        if [ -z "$now" ]; then
            exec systemctl --no-block disable --now "$UNIT"
        elif [ "$now" != "$started" ]; then
            exec systemctl --no-block restart "$UNIT"
        fi
    done &
fi

# flatbox runs this through `sh -c`, so arguments added after wyre-daemon get
# split and expanded
exec "$bin_dir/flatbox" run --flatpak-install-path "$installation" \
    --app dev.skepy.wyre --apparmor-unconfined wyre-daemon