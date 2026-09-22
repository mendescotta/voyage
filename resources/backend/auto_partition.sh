#!/bin/bash

DISK="$1"
TARGET="/mnt/target"

if [ -z "$DISK" ]; then
    echo "Usage: $0 /dev/sdX"
    exit 1
fi

# Confirm EFI (same check as backend_install.sh, so the partition table
# type created here always matches the grub-install target chosen later)
EFI=0
if [ -e /sys/firmware/efi/systab ]; then
    EFI=1
fi

# Warning: wipes EVERYTHING on the disk
sgdisk --zap-all "$DISK"

if [ $EFI -eq 1 ]; then
    sgdisk -o "$DISK"
    sgdisk -n 1:2048:+512M -t 1:ef00 "$DISK"
    sgdisk -n 2:0:0 -t 2:8300 "$DISK"
else
    parted -s "$DISK" mklabel msdos
    parted -s "$DISK" mkpart primary 1MiB 100%
fi

echo "Automatic partitioning completed on $DISK"
