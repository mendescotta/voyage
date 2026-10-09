#!/usr/bin/env bash
# The backend applies distro.conf: login shell, extra packages, services; and the hook stages sit at the
# right places of the install flow.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^enable_service()/,/^}/p;/^disable_service()/,/^}/p;/^apply_distro_services()/,/^}/p;/^install_distro_packages()/,/^}/p;/^set_default_shell()/,/^}/p' "$SCRIPT")"
for f in enable_service disable_service apply_distro_services install_distro_packages set_default_shell; do
	type "$f" >/dev/null 2>&1 || { echo "FAIL $f is not defined in the backend"; exit 1; }
done

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

DISTRO_CONF_DEFAULT="$T/distro.conf"; DISTRO_CONF_OVERRIDE=/nonexistent
. "$HERE/resources/backend/distro_config.sh"
TARGETDIR="$T/target"; CALLS=""; INSTALLED=" "; OPTS_UPDATE=1; OPTS_USERLOGIN=gui
mkdir -p "$TARGETDIR/etc"
log_ui() { :; }
get_option() { case "$1" in UPDATE) echo "$OPTS_UPDATE" ;; USERLOGIN) echo "$OPTS_USERLOGIN" ;; esac; }
chroot() {
	shift
	case "$1" in
		xbps-query) case "$INSTALLED" in *" $2 "*) return 0 ;; *) return 1 ;; esac ;;
		xbps-install) CALLS="$CALLS|$*"; [ "${OFFLINE:-}" = 1 ] && return 1; return 0 ;;
		usermod) CALLS="$CALLS|$*" ;;
	esac
}

# ---- the shipped default behaves like the installer always did
DISTRO_CONF_DEFAULT="$HERE/resources/backend/distro.conf"
set_default_shell
check "default: fish is installed when missing and set for root and the user" "$CALLS" "|xbps-install -Sy fish-shell|usermod -s /usr/bin/fish root|usermod -s /usr/bin/fish gui"
check "default: /etc/shells lists it" "$(grep -c '^/usr/bin/fish$' "$TARGETDIR/etc/shells")" "1"

CALLS=""; INSTALLED=" fish-shell "
set_default_shell
check "an installed shell is not installed again" "$CALLS" "|usermod -s /usr/bin/fish root|usermod -s /usr/bin/fish gui"
check "/etc/shells is not duplicated" "$(grep -c '^/usr/bin/fish$' "$TARGETDIR/etc/shells")" "1"

CALLS=""; INSTALLED=" "; OFFLINE=1
set_default_shell
check "offline and missing: the shell is left alone" "$CALLS" "|xbps-install -Sy fish-shell"
OFFLINE=0

# ---- a distro that wants bash, or the live image's shell
printf 'default-shell /bin/bash\n' > "$T/distro.conf"; DISTRO_CONF_DEFAULT="$T/distro.conf"; CALLS=""
set_default_shell
check "a shell without a package is only set" "$CALLS" "|usermod -s /bin/bash root|usermod -s /bin/bash gui"
printf 'default-shell none\n' > "$T/distro.conf"; CALLS=""
set_default_shell
check "none leaves the live image's shell" "$CALLS" ""

# ---- extra packages
printf 'install nano htop\n' > "$T/distro.conf"; CALLS=""
install_distro_packages
check "extra packages are installed in one call" "$CALLS" "|xbps-install -Sy nano htop"
CALLS=""; OPTS_UPDATE=0; install_distro_packages
check "offline installs skip them" "$CALLS" ""
OPTS_UPDATE=1; printf '# nothing\n' > "$T/distro.conf"; CALLS=""; install_distro_packages
check "no packages: nothing runs" "$CALLS" ""

# ---- services, on both inits
printf 'enable sshd\nenable ghost\ndisable dhcpcd\n' > "$T/distro.conf"
INIT_SYSTEM=dinit; rm -rf "$TARGETDIR/etc"; mkdir -p "$TARGETDIR/etc/dinit.d/boot.d"
touch "$TARGETDIR/etc/dinit.d/sshd" "$TARGETDIR/etc/dinit.d/dhcpcd"; ln -s /etc/dinit.d/dhcpcd "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd"
apply_distro_services 2>"$T/err"
check "dinit: a service is enabled"   "$(readlink "$TARGETDIR/etc/dinit.d/boot.d/sshd")" "/etc/dinit.d/sshd"
check "dinit: a service is disabled"  "$([ -e "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd" ] || [ -L "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd" ] && echo present || echo gone)" "gone"
check "dinit: a missing service is skipped with a warning" "$(grep -c 'no service ghost' "$T/err")" "1"
check "dinit: and no dangling link is made" "$([ -L "$TARGETDIR/etc/dinit.d/boot.d/ghost" ] && echo link || echo none)" "none"

INIT_SYSTEM=runit; rm -rf "$TARGETDIR/etc"; mkdir -p "$TARGETDIR/etc/sv/sshd" "$TARGETDIR/etc/runit/runsvdir/default"
apply_distro_services 2>/dev/null
check "runit: a service is enabled"   "$(readlink "$TARGETDIR/etc/runit/runsvdir/default/sshd")" "/etc/sv/sshd"

# ---- the stages sit where the flow needs them
line() { grep -n "$1" "$SCRIPT" | tail -1 | cut -d: -f1; }
order_ok() { [ -n "$1" ] && [ -n "$2" ] && [ "$1" -lt "$2" ]; }
conf=$(line '^distro_conf_check ||'); fs=$(line '^create_filesystems$'); pre=$(line 'run_hooks pre-copy'); cp_=$(line '^copy_rootfs$')
post=$(line 'run_hooks post-copy'); dracut=$(line 'chroot "\$TARGETDIR" dracut'); rm_=$(line '^remove_installer_packages$'); fin=$(line 'run_hooks post-install'); umount_=$(line '^umount_filesystems$')
order_ok "$conf" "$fs"   && echo "ok   distro.conf is checked before anything is created"   || { echo "FAIL conf check ($conf) before create_filesystems ($fs)"; fails=$((fails + 1)); }
order_ok "$pre" "$cp_"   && echo "ok   pre-copy runs before the copy"                        || { echo "FAIL pre-copy ($pre) before copy ($cp_)"; fails=$((fails + 1)); }
order_ok "$post" "$dracut" && echo "ok   post-copy runs before the initramfs is rebuilt"     || { echo "FAIL post-copy ($post) before dracut ($dracut)"; fails=$((fails + 1)); }
order_ok "$rm_" "$fin"   && echo "ok   post-install runs after the installer is removed"    || { echo "FAIL post-install ($fin) after removal ($rm_)"; fails=$((fails + 1)); }
order_ok "$fin" "$umount_" && echo "ok   and before the target is unmounted"                 || { echo "FAIL post-install ($fin) before umount ($umount_)"; fails=$((fails + 1)); }

[ "$fails" -eq 0 ]
