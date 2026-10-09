# Sourced by backend_install.sh and auto_partition.sh. The installer must never touch the disk the live
# system booted from: it holds the running system, and wiping it would end the installation (and the USB
# stick). The UI hides that disk; this is the second line, for anything that reaches the scripts anyway.

# top-level disks (one per line) that carry the live medium's mount
live_medium_disks() {
    local mp src
    for mp in /run/initramfs/live /run/live/medium /run/initramfs/medium; do
        src="$(findmnt -n -o SOURCE "$mp" 2>/dev/null)" || continue
        [ -n "$src" ] || continue
        # -s walks from the device up through its parents (partition, crypt/dm layers) to the disk
        lsblk -nrso NAME,TYPE "$src" 2>/dev/null | awk '$2 == "disk" { print "/dev/" $1 }'
    done | sort -u
}

# refuse_live_medium <device>: fails (with the reason on stderr) when <device> is, or sits on, the live disk
refuse_live_medium() {
    local dev="$1" live disk
    live="$(live_medium_disks)"
    [ -n "$live" ] || return 0
    for disk in $(lsblk -nrso NAME,TYPE "$dev" 2>/dev/null | awk '$2 == "disk" { print "/dev/" $1 }'); do
        if printf '%s\n' "$live" | grep -qxF -- "$disk"; then
            echo "Refusing to use $dev: it is on $disk, the medium this system booted from." >&2
            return 1
        fi
    done
    return 0
}
