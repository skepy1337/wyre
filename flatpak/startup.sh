#!/bin/sh

SOCKET=/run/wyre/wyre.sock
UNIT=wyre-daemon.service

host() {
    flatpak-spawn --host "$@"
}

dialog() {
    kdialog --title Wyre --icon dev.skepy.wyre "$@"
}

error() {
    dialog --error "$1"
}

setup() {
    # through the "active" link, so the service runs the installed version
    commit=$(sed -n 's/^app-commit=//p' /.flatpak-info)
    bin=$(sed -n 's/^app-path=//p' /.flatpak-info | sed "s|/$commit/|/active/|")/bin

    # root runs these files, so only root may be able to change them
    if ! problem=$(host sh "$bin/daemon.sh" --check 2>&1); then
        error "<p>Wyre's service can only be set up when Wyre is installed system-wide:</p>
<p><tt>flatpak install --system dev.skepy.wyre</tt></p>
<p><small>$problem</small></p>"
        return
    fi

    if host sh -c 'command -v pkexec' >/dev/null; then root=pkexec; else root=run0; fi

    unit="[Unit]
Description=Wyre network traffic daemon
RequiresMountsFor=\"$bin\"
ConditionPathExists=$bin/daemon.sh

[Service]
ExecStartPre=-/bin/sh -c \"chcon -t bin_t '$bin/daemon.sh' '$bin/flatbox' 2>/dev/null\"
ExecStart=/bin/sh \"$bin/daemon.sh\"
Restart=on-failure

[Install]
WantedBy=multi-user.target"

    if [ "$(host cat "/etc/systemd/system/$UNIT" 2>/dev/null)" = "$unit" ]; then
        dialog --yesno "Wyre's service isn't running. Start it?" \
            --yes-label Start --no-label Cancel || return
        host "$root" systemctl restart "$UNIT" ||
            { error "The service couldn't be started."; return; }
    else
        dialog --yesnocancel "<p>Wyre tracks network traffic with a service that \
runs as root, outside the Flatpak sandbox.</p>
<p>Install <tt>$UNIT</tt>? It can start with your computer, or only now.</p>" \
            --yes-label "Start at Boot" --no-label "Start Now Only"
        case $? in
        0) boot=TRUE ;;
        1) boot=FALSE ;;
        *) return ;;
        esac

        # the unit goes to root as an argument, as anything running as you
        # could swap a file before root copied it
        host "$root" sh -c '
            set -e
            printf "%s\n" "$1" > "/etc/systemd/system/$2"
            chmod 644 "/etc/systemd/system/$2"
            echo "d /run/wyre 0755 root root -" > /etc/tmpfiles.d/wyre.conf
            systemctl daemon-reload
            if [ "$3" = TRUE ]; then systemctl enable "$2"; fi
            systemctl restart "$2"' sh "$unit" "$UNIT" "$boot" ||
            { error "The service couldn't be set up."; return; }
    fi

    # the daemon takes a moment to load its eBPF programs
    for _ in $(seq 20); do
        host test -S "$SOCKET" && break
        sleep 0.5
    done
    if ! host test -S "$SOCKET"; then
        error "<p>The service didn't start. <tt>journalctl -u $UNIT</tt> may say why.</p>"
    elif [ ! -S "$SOCKET" ]; then
        # the sandbox only shares /run/wyre if it existed when it started
        dialog --msgbox "The service is running. Please start Wyre again."
        exit 0
    fi
}

[ -S "$SOCKET" ] || setup
exec wyre "$@"