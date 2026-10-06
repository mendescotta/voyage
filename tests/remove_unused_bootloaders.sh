#!/usr/bin/env bash
# Choosing one bootloader must remove every package of the others, including the
# grub-*-efi packages that depend on grub (xbps-remove without -R refuses otherwise).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^remove_unused_bootloaders()/,/^}/p;/^set_bootloader()/,/^}/p' "$SCRIPT")"
type set_bootloader >/dev/null 2>&1 || { echo "FAIL set_bootloader is not defined in the backend"; exit 1; }

TARGETDIR=/target
CALLS=""
INSTALLED=" grub grub-x86_64-efi grub-i386-efi refind limine "
chroot() {
	shift
	case "$1" in
		xbps-query) case "$INSTALLED" in *" $2 "*) return 0 ;; *) return 1 ;; esac ;;
		xbps-remove) CALLS="$CALLS|$*" ;;
	esac
}
get_option() { case "$1" in BOOTLOADER) echo /dev/sda ;; BOOTLOADER_TYPE) echo "$BL" ;; esac; }
install_grub() { :; }; install_refind() { :; }; install_limine() { :; }

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

BL=refind; CALLS=""; set_bootloader
check "refind removes grub, its EFI packages and limine" "$CALLS" "|xbps-remove -Ry grub grub-x86_64-efi grub-i386-efi limine"

BL=limine; CALLS=""; set_bootloader
check "limine removes grub, its EFI packages and refind" "$CALLS" "|xbps-remove -Ry grub grub-x86_64-efi grub-i386-efi refind"

BL=grub; CALLS=""; set_bootloader
check "grub removes limine and refind" "$CALLS" "|xbps-remove -Ry limine refind"

BL=refind; INSTALLED=" refind "; CALLS=""; set_bootloader
check "nothing to remove means no xbps-remove call" "$CALLS" ""

[ "$fails" -eq 0 ]
