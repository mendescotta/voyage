#!/usr/bin/env bash
# Only installer packages that are actually installed in the target may be passed
# to xbps-remove (otherwise xbps logs "<pkg> is not installed" noise).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^INSTALLER_ONLY_PKGS=/p;/^remove_installer_packages()/,/^}/p' "$SCRIPT")"
type remove_installer_packages >/dev/null 2>&1 || { echo "FAIL remove_installer_packages is not defined in the backend"; exit 1; }

TARGETDIR=/target
CALLS=""
INSTALLED=" voyage xmirror dialog "
chroot() {
	shift
	case "$1" in
		xbps-query) case "$INSTALLED" in *" $2 "*) return 0 ;; *) return 1 ;; esac ;;
		xbps-remove) CALLS="$CALLS|$*" ;;
	esac
}

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

remove_installer_packages
check "removes only the installed installer packages" "$CALLS" "|xbps-remove -ROoy voyage xmirror dialog"

CALLS=""; INSTALLED=" "
remove_installer_packages
check "calls xbps-remove not at all when none are installed" "$CALLS" ""

CALLS=""; INSTALLED=" xtools-minimal "
remove_installer_packages
check "removes xtools-minimal when it is installed" "$CALLS" "|xbps-remove -ROoy xtools-minimal"

[ "$fails" -eq 0 ]
