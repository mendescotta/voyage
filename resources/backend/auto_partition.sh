#!/bin/bash

DISK="$1"
LAYOUT="${2:-basic}"
TARGET="/mnt/target"

if [ -z "$DISK" ]; then
    echo "Usage: $0 /dev/sdX [basic|with-swap]"
    exit 1
fi

# Confirm EFI (same check as backend_install.sh, so the partition table
# type created here always matches the grub-install target chosen later)
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

# Warning: wipes EVERYTHING on the disk
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
    else
        sgdisk -n 2:0:0 -t 2:8300 "$DISK"
    fi
else
    parted -s "$DISK" mklabel msdos
    if [ "$LAYOUT" = "with-swap" ]; then
        parted -s "$DISK" mkpart primary linux-swap 1MiB "${swap_mib}MiB"
        parted -s "$DISK" mkpart primary "${swap_mib}MiB" 100%
    else
        parted -s "$DISK" mkpart primary 1MiB 100%
    fi
fi

echo "Automatic partitioning completed on $DISK ($LAYOUT layout)"
