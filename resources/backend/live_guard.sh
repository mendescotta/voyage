# Sourced by backend_install.sh and auto_partition.sh. The installer must never touch the disk the live
# system booted from: it holds the running system, and wiping it would end the installation (and the USB
# stick). The UI hides that disk; this is the second line, for anything that reaches the scripts anyway.

# Physical devices carrying a mount, including optical drives used by VM ISOs.
# findmnt may append a bind-mount subdirectory (SOURCE[/path]); it is not part of the device name.
live_source_devices() {
    local src="${1%%\[*}" back depth="${2:-0}"
    [ "$depth" -lt 16 ] || return 1
    case "$src" in
        /dev/loop*)
            back="$(losetup -nO BACK-FILE "$src" 2>/dev/null)" || return 1
            [ -n "$back" ] || return 1
            src="$(findmnt -T "$back" -nro SOURCE 2>/dev/null)" || return 1
            live_source_devices "$src" "$((depth + 1))"
            ;;
        /dev/*)
            lsblk -nrso NAME,TYPE "$src" 2>/dev/null |
                awk '$2 == "disk" || $2 == "rom" { print "/dev/" $1 }'
            ;;
        *) return 1 ;;
    esac
}

# One device per line. An unknown mounted live medium is an error, not an empty safe list.
live_medium_disks() {
    local mp src devices all="" unresolved=0
    for mp in /run/initramfs/live /run/live/medium /run/initramfs/medium; do
        src="$(findmnt -nro SOURCE --mountpoint "$mp" 2>/dev/null)" || continue
        [ -n "$src" ] || continue
        devices="$(live_source_devices "$src")"
        if [ -z "$devices" ]; then
            echo "Refusing installation: $mp is mounted but its backing device cannot be determined." >&2
            unresolved=1
        else
            all="${all}${devices}
"
        fi
    done
    [ "$unresolved" -eq 0 ] || return 1
    [ -z "$all" ] || printf '%s' "$all" | sort -u
    return 0
}

# refuse_live_medium <device>: fails (with the reason on stderr) when <device> is, or sits on, the live disk
refuse_live_medium() {
    local dev="$1" live disk found=0
    # callers that check several devices resolve the live disks once and set LIVE_DISKS (no repeated work or
    # warnings); a plain call resolves them itself
    if [ -n "${LIVE_DISKS+set}" ]; then live="$LIVE_DISKS"; else live="$(live_medium_disks)" || return 1; fi
    [ -n "$live" ] || return 0
    for disk in $(live_source_devices "$dev"); do
        found=1
        if printf '%s\n' "$live" | grep -qxF -- "$disk"; then
            echo "Refusing to use $dev: it is on $disk, the medium this system booted from." >&2
            return 1
        fi
    done
    if [ "$found" -eq 0 ]; then
        # a live medium is known but this device cannot be placed on a disk: do not guess
        echo "Refusing to use $dev: it cannot be traced to a disk, so it cannot be shown to be off the live medium." >&2
        return 1
    fi
    return 0
}
