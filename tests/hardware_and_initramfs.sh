#!/usr/bin/env bash
# Hardware drivers are installed by voidhw against the target (never fatal), and the initramfs
# driver set maps to dracut options: generic by default, --hostonly for targeted.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^initramfs_dracut_args()/,/^}/p;/^install_hardware_drivers()/,/^}/p' "$SCRIPT")"
type initramfs_dracut_args >/dev/null 2>&1 || { echo "FAIL initramfs_dracut_args is not defined in the backend"; exit 1; }
type install_hardware_drivers >/dev/null 2>&1 || { echo "FAIL install_hardware_drivers is not defined in the backend"; exit 1; }

TARGETDIR=/target
CALLS=""
UI=""
log_ui() { UI="$UI|$*"; }
get_option() { case "$1" in HWDRIVERS) echo "$HW" ;; DRIVERSET) echo "$SET" ;; esac; }
voidhw() { CALLS="$CALLS|voidhw $*"; return "$VOIDHW_STATUS"; }

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

SET=generic;  check "generic is the default driver set" "$(initramfs_dracut_args)" "--no-hostonly --add-drivers ahci"
SET="";       check "an unset driver set is generic"     "$(initramfs_dracut_args)" "--no-hostonly --add-drivers ahci"
SET=targeted; check "targeted uses dracut --hostonly"    "$(initramfs_dracut_args)" "--hostonly"

HW=1; VOIDHW_STATUS=0; CALLS=""; UI=""
install_hardware_drivers
check "voidhw configures the target for the running hardware" "$CALLS" "|voidhw --apply --root /target --hardware-from /"
check "the installer shows the hardware step" "$UI" "|HARDWARE"

HW=0; CALLS=""; UI=""
install_hardware_drivers >/dev/null
check "nothing runs when the user declined" "$CALLS$UI" ""

HW=1; VOIDHW_STATUS=1; CALLS=""
install_hardware_drivers >/dev/null 2>&1; check "a voidhw failure does not abort the install" "$?" "0"

[ "$fails" -eq 0 ]
