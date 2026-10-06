#!/usr/bin/env bash
# The install log is copied to the new user's desktop, and the rEFInd theme goes next to the
# refind.conf that rEFInd actually reads (EFI/BOOT with --usedefault, EFI/refind otherwise).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^copy_log_to_desktop()/,/^}/p;/^refind_install_dir()/,/^}/p' "$SCRIPT")"
type copy_log_to_desktop >/dev/null 2>&1 || { echo "FAIL copy_log_to_desktop is not defined in the backend"; exit 1; }
type refind_install_dir >/dev/null 2>&1 || { echo "FAIL refind_install_dir is not defined in the backend"; exit 1; }

W="$(mktemp -d)"
trap 'rm -rf --one-file-system "$W"' EXIT
TARGETDIR="$W/target"
LOG="$W/installation.log"
echo "log line" >"$LOG"
LOGIN=anna
get_option() { [ "$1" = USERLOGIN ] && echo "$LOGIN"; }
chroot() {
	shift
	case "$1 $2" in
		"id -u") id -u ;;
		"id -g") id -g ;;
	esac
}

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

mkdir -p "$TARGETDIR/home/anna"
copy_log_to_desktop
check "log lands on the user's desktop" "$(cat "$TARGETDIR/home/anna/Desktop/installation.log" 2>/dev/null)" "log line"

rm -rf "$TARGETDIR/home/anna/Desktop"; LOGIN=""
copy_log_to_desktop
check "no user means no desktop copy" "$(ls "$TARGETDIR/home/anna" | wc -l | tr -d ' ')" "0"

mkdir -p "$W/esp/BOOT" "$W/esp/refind"
touch "$W/esp/BOOT/refind.conf"
check "--usedefault install: theme dir is EFI/BOOT" "$(refind_install_dir "$W/esp")" "$W/esp/BOOT"
rm "$W/esp/BOOT/refind.conf"; touch "$W/esp/refind/refind.conf"
check "plain install: theme dir is EFI/refind" "$(refind_install_dir "$W/esp")" "$W/esp/refind"
rm "$W/esp/refind/refind.conf"
refind_install_dir "$W/esp" >/dev/null; check "no refind.conf is an error" "$?" "1"

[ "$fails" -eq 0 ]
