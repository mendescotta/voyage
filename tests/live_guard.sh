#!/usr/bin/env bash
# The backend must never touch the disk the live system booted from, whatever it is asked to install to.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUARD="$HERE/resources/backend/live_guard.sh"
[ -f "$GUARD" ] || { echo "FAIL $GUARD does not exist"; exit 1; }

LIVE_SRC=""
LOOP_BACK=""
losetup() {
    if [ "${*: -1}" = /dev/loop1 ] && [ -n "${NESTED_BACK:-}" ]; then
        echo "$NESTED_BACK"
    else
        [ -n "$LOOP_BACK" ] && echo "$LOOP_BACK"
    fi
}
findmnt() { # findmnt -n -o SOURCE <mountpoint>   |   findmnt -T <file> -no SOURCE
	case " $* " in
        *" -T "*)
            if [[ " $* " == *" /nested/outer.iso "* ]]; then echo /dev/sdb2
            elif [ "$LOOP_BACK" = /isos/live.iso ]; then echo "${BACK_SRC:-/dev/sdb2}"
            fi
            return 0 ;;
    esac
	local mp="${*: -1}"
    if [ "$mp" = /run/live/medium ] && [ -n "${LIVE_ALT_SRC:-}" ]; then
        echo "$LIVE_ALT_SRC"; return 0
    fi
	[ "$mp" = "/run/initramfs/live" ] && [ -n "$LIVE_SRC" ] && { echo "$LIVE_SRC"; return 0; }
	return 1
}
lsblk() { # lsblk -nrso NAME,TYPE <dev>: the device and its parents, one per line
	case "${*: -1}" in
		/dev/sda)       printf 'sda disk\n' ;;
		/dev/sda1)      printf 'sda1 part\nsda disk\n' ;;
		/dev/sda2)      printf 'sda2 part\nsda disk\n' ;;
		/dev/nvme0n1)   printf 'nvme0n1 disk\n' ;;
		/dev/nvme0n1p1) printf 'nvme0n1p1 part\nnvme0n1 disk\n' ;;
		/dev/mapper/live) printf 'live crypt\nsdb1 part\nsdb disk\n' ;;
		/dev/sdb)       printf 'sdb disk\n' ;;
		/dev/sdb2)      printf 'sdb2 part\nsdb disk\n' ;;
		/dev/sr0)       printf 'sr0 rom\n' ;;
		/dev/loop1)     printf 'loop1 loop\n' ;;
		/dev/loop0)     printf 'loop0 loop\n' ;;
		*) return 1 ;;
	esac
}
. "$GUARD"

fails=0
refuses() { if refuse_live_medium "$2" >/dev/null 2>&1; then echo "FAIL $1: $2 was accepted"; fails=$((fails + 1)); else echo "ok   $1"; fi; }
allows()  { if refuse_live_medium "$2" >/dev/null 2>&1; then echo "ok   $1"; else echo "FAIL $1: $2 was refused"; fails=$((fails + 1)); fi; }

LIVE_SRC=/dev/sda1
refuses "the live partition's own disk" /dev/sda
refuses "the live partition itself"     /dev/sda1
refuses "another partition of the live disk" /dev/sda2
allows  "a different disk"              /dev/nvme0n1
allows  "a partition of a different disk" /dev/nvme0n1p1

LIVE_SRC=/dev/mapper/live
refuses "a medium behind a mapper device: its disk" /dev/sdb
allows  "a medium behind a mapper device: others"   /dev/sda

LIVE_SRC=/dev/loop0; LOOP_BACK=/isos/live.iso
refuses "an ISO loop-mounted from a file: the disk that holds the file" /dev/sdb
allows  "an ISO loop-mounted from a file: other disks"               /dev/sda
LOOP_BACK=""

LIVE_SRC=/dev/sr0
refuses "an optical live ISO: the optical drive itself" /dev/sr0
allows "an optical live ISO: the installation disk" /dev/sda
[ "$(live_medium_disks)" = /dev/sr0 ] || { echo "FAIL optical live medium not resolved"; fails=$((fails + 1)); }
LIVE_SRC='/dev/sda1[/LiveOS]'
refuses "a bind-mounted live medium: its backing disk" /dev/sda
allows "a bind-mounted live medium: other disks" /dev/sdb
LIVE_SRC=/dev/loop0; LOOP_BACK=/isos/live.iso; BACK_SRC='/dev/sdb2[/isos]'
refuses "a loop backing file on a bind mount" /dev/sdb
BACK_SRC=/dev/loop1; NESTED_BACK=/nested/outer.iso
refuses "nested loop-mounted ISOs: the physical backing disk" /dev/sdb
allows "nested loop-mounted ISOs: other disks" /dev/sda
unset NESTED_BACK
refuses "cyclic loop backing devices fail closed" /dev/sda
unset BACK_SRC
LOOP_BACK=""

LIVE_SRC=/dev/loop0
msg="$(refuse_live_medium /dev/sda 2>&1 >/dev/null)"; rc=$?
[ "$rc" -ne 0 ] && echo "ok   an unresolvable live mount blocks installation" || { echo "FAIL unresolvable mount accepted"; fails=$((fails + 1)); }
case "$msg" in *"cannot"*"/run/initramfs/live"*|*"/run/initramfs/live"*"cannot"*) echo "ok   the error identifies the unresolved live mount" ;; *) echo "FAIL no warning for an unresolvable live mount: '$msg'"; fails=$((fails + 1)) ;; esac

LIVE_SRC=/dev/sda1; LIVE_ALT_SRC=/dev/sdb2
refuses "a second live mount protects its disk too" /dev/sdb
allows "multiple live mounts still permit a separate target" /dev/nvme0n1
LIVE_ALT_SRC=/dev/loop0
refuses "an unresolved second mount blocks despite a known first disk" /dev/nvme0n1
unset LIVE_ALT_SRC

# a device the guard cannot resolve is refused while a live medium is known (fail closed)
LIVE_SRC=/dev/sda1
refuses "an unresolvable device while the live disk is known" /dev/does-not-exist

# the live disks can be resolved once and reused: no repeated work, no repeated warnings
LIVE_SRC=/dev/loop0
LIVE_DISKS=""
n=$( { refuse_live_medium /dev/sda; refuse_live_medium /dev/nvme0n1; refuse_live_medium /dev/sda1; } 2>&1 >/dev/null | grep -c "cannot be determined")
if [ "$n" = "0" ]; then echo "ok   with a precomputed (empty) list an explicitly empty cached list does not trigger another lookup"; else echo "FAIL the warning was repeated $n times"; fails=$((fails + 1)); fi
unset LIVE_DISKS
LIVE_SRC=/dev/sda1; LIVE_DISKS="/dev/nvme0n1"
refuses "a precomputed list is what is compared against" /dev/nvme0n1
allows  "and other disks pass" /dev/sda
unset LIVE_DISKS

LIVE_SRC=""
allows  "no live mount (installing from an installed system)" /dev/sda
allows  "no live mount: any partition" /dev/sda1

LIVE_SRC=/dev/sda1
out="$(refuse_live_medium /dev/sda2 2>&1)"
case "$out" in *"/dev/sda2"*"/dev/sda"*) echo "ok   the message names the device and the live disk" ;; *) echo "FAIL message: $out"; fails=$((fails + 1)) ;; esac

# wiring: both scripts source the guard and the check comes before any destructive step
for f in backend_install.sh auto_partition.sh; do
	if grep -q 'live_guard.sh' "$HERE/resources/backend/$f"; then echo "ok   $f sources the guard"; else echo "FAIL $f does not source the guard"; fails=$((fails + 1)); fi
done
first_check=$(grep -n 'check_not_live_medium$' "$HERE/resources/backend/backend_install.sh" | tail -1 | cut -d: -f1)
first_write=$(grep -n '^create_filesystems$' "$HERE/resources/backend/backend_install.sh" | head -1 | cut -d: -f1)
if [ -n "$first_check" ] && [ -n "$first_write" ] && [ "$first_check" -lt "$first_write" ]; then echo "ok   the check runs before the file systems are created"; else echo "FAIL check ($first_check) must precede create_filesystems ($first_write)"; fails=$((fails + 1)); fi
a=$(grep -n 'refuse_live_medium "\$DISK"' "$HERE/resources/backend/auto_partition.sh" | head -1 | cut -d: -f1)
b=$(grep -n -E 'wipefs|sgdisk|parted|sfdisk|mkfs' "$HERE/resources/backend/auto_partition.sh" | head -1 | cut -d: -f1)
if [ -n "$a" ] && [ -n "$b" ] && [ "$a" -lt "$b" ]; then echo "ok   auto_partition refuses before it partitions"; else echo "FAIL guard ($a) must precede the first partitioning tool ($b)"; fails=$((fails + 1)); fi

[ "$fails" -eq 0 ]
