#!/bin/bash

DISK="$1"
LAYOUT="${2:-basic}"
SHRED="${3:-}"
TARGET="/mnt/target"

if [ -z "$DISK" ]; then
    echo "Usage: $0 /dev/sdX [basic|with-swap] [--shred]"
    exit 1
fi

EFI=0
if [ -e /sys/firmware/efi/systab ]; then
    EFI=1
fi

swap_size_mib() {
    local mem_kib mem_mib size_mib
    mem_kib="$(awk '/MemTotal/ {print $2}' /proc/meminfo)"
    mem_mib=$(( mem_kib / 1024 ))
    size_mib=$mem_mib
    [ "$size_mib" -gt 8192 ] && size_mib=8192
    [ "$size_mib" -lt 256 ] && size_mib=256
    echo "$size_mib"
}

partition_dev() {
    local disk="$1" num="$2"
    case "$disk" in
        *nvme*|*mmcblk*|*nbd*) echo "${disk}p${num}" ;;
        *) echo "${disk}${num}" ;;
    esac
}

close_stale_lvm() {
    local disk="$1" pv vg
    command -v vgchange >/dev/null 2>&1 || return 0
    vgscan --mknodes >/dev/null 2>&1 || true
    pvscan --cache >/dev/null 2>&1 || true
    for pv in $(lsblk -lnpo NAME,FSTYPE "$disk" 2>/dev/null | awk '$2=="LVM2_member"{print $1}'); do
        vg="$(pvs --noheadings -o vg_name "$pv" 2>/dev/null | tr -d ' ')"
        if [ -n "$vg" ]; then
            vgchange -an "$vg" >/dev/null 2>&1 || echo "WARNING: could not deactivate volume group $vg on $pv" >&2
        fi
    done
}

close_stale_lvm "$DISK"

if [ "$SHRED" = "--shred" ]; then
    echo "Securely erasing $DISK (single pass of zeros)..."
    shred -n0 -z "$DISK" || echo "WARNING: shred failed on $DISK, continuing with partitioning anyway" >&2
fi

sgdisk --zap-all "$DISK"

if [ "$LAYOUT" = "with-swap" ]; then
    swap_mib="$(swap_size_mib)"
fi

if [ $EFI -eq 1 ]; then
    sgdisk -o "$DISK"
    sgdisk -n 1:2048:+512M -t 1:ef00 "$DISK"
    if [ "$LAYOUT" = "with-swap" ]; then
        sgdisk -n 2:0:+"${swap_mib}"M -t 2:8200 "$DISK"
        sgdisk -n 3:0:0 -t 3:8300 "$DISK"
        echo "SWAP_PARTITION=$(partition_dev "$DISK" 2)"
    else
        sgdisk -n 2:0:0 -t 2:8300 "$DISK"
    fi
else
    parted -s "$DISK" mklabel msdos
    if [ "$LAYOUT" = "with-swap" ]; then
        parted -s "$DISK" mkpart primary linux-swap 1MiB "${swap_mib}MiB"
        parted -s "$DISK" mkpart primary "${swap_mib}MiB" 100%
        echo "SWAP_PARTITION=$(partition_dev "$DISK" 1)"
    else
        parted -s "$DISK" mkpart primary 1MiB 100%
    fi
fi

echo "Automatic partitioning completed on $DISK ($LAYOUT layout)"
